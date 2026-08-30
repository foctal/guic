use crate::{Label, ScrollArea};
use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, Keystroke,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::{error::Error, fmt, rc::Rc};

use crate::virtual_list::VirtualListMetrics;

type SharedStringHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type TreeSelectionHandler = Rc<dyn Fn(&TreeSelection, &mut Window, &mut App)>;

/// External viewport metadata for virtualized [`TreeView`] rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeViewport {
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
}

impl TreeViewport {
    /// Creates a viewport descriptor.
    #[must_use]
    pub fn new(scroll_offset: f32, viewport_height: f32) -> Self {
        Self {
            scroll_offset: if scroll_offset.is_finite() {
                scroll_offset.max(0.0)
            } else {
                0.0
            },
            viewport_height: if viewport_height.is_finite() {
                viewport_height.max(0.0)
            } else {
                0.0
            },
            overscan: 4,
        }
    }

    /// Sets the number of extra rows rendered above and below the viewport.
    #[must_use]
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Returns the vertical scroll offset in logical pixels.
    #[must_use]
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Returns the viewport height in logical pixels.
    #[must_use]
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Returns the overscan row count.
    #[must_use]
    pub fn overscan_rows(&self) -> usize {
        self.overscan
    }
}

/// A controlled update applied to one existing [`TreeNode`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeMutation {
    node_id: SharedString,
    kind: TreeMutationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TreeMutationKind {
    Expanded(bool),
    Selected(bool),
    Checked(bool),
    Loading(bool),
    ReplaceChildren(Vec<TreeNode>),
}

impl TreeMutation {
    /// Creates an expansion-state update.
    #[must_use]
    pub fn expanded(node_id: impl Into<SharedString>, expanded: bool) -> Self {
        Self {
            node_id: node_id.into(),
            kind: TreeMutationKind::Expanded(expanded),
        }
    }

    /// Creates a selection-state update.
    #[must_use]
    pub fn selected(node_id: impl Into<SharedString>, selected: bool) -> Self {
        Self {
            node_id: node_id.into(),
            kind: TreeMutationKind::Selected(selected),
        }
    }

    /// Creates a checkbox-state update.
    #[must_use]
    pub fn checked(node_id: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            node_id: node_id.into(),
            kind: TreeMutationKind::Checked(checked),
        }
    }

    /// Creates a lazy-loading-state update.
    #[must_use]
    pub fn loading(node_id: impl Into<SharedString>, loading: bool) -> Self {
        Self {
            node_id: node_id.into(),
            kind: TreeMutationKind::Loading(loading),
        }
    }

    /// Creates an update that replaces a node's complete child collection.
    #[must_use]
    pub fn replace_children(node_id: impl Into<SharedString>, children: Vec<TreeNode>) -> Self {
        Self {
            node_id: node_id.into(),
            kind: TreeMutationKind::ReplaceChildren(children),
        }
    }

    /// Returns the target node identifier.
    #[must_use]
    pub fn node_id(&self) -> &SharedString {
        &self.node_id
    }
}

/// Error returned when a controlled tree mutation cannot be applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeMutationError {
    node_id: SharedString,
}

impl TreeMutationError {
    /// Returns the missing target node identifier.
    #[must_use]
    pub fn node_id(&self) -> &SharedString {
        &self.node_id
    }
}

impl fmt::Display for TreeMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tree node '{}' was not found", self.node_id)
    }
}

impl Error for TreeMutationError {}

/// Flattened visible node metadata for host-managed tree interactions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleTreeNode {
    id: SharedString,
    depth: usize,
    has_children: bool,
    expanded: bool,
    selected: bool,
    checked: bool,
    loading: bool,
}

impl VisibleTreeNode {
    fn new(node: &TreeNode, depth: usize) -> Self {
        Self {
            id: node.id.clone(),
            depth,
            has_children: node.has_children(),
            expanded: node.expanded,
            selected: node.selected,
            checked: node.checked,
            loading: node.loading,
        }
    }

    /// Returns the stable node identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the visible depth of the node.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Returns whether the node has child nodes.
    #[must_use]
    pub fn has_children(&self) -> bool {
        self.has_children
    }

    /// Returns whether the node is currently expanded.
    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Returns whether the node is currently selected.
    #[must_use]
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Returns whether the node is checked in checkbox selection mode.
    #[must_use]
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Returns whether the node is showing a lazy-loading placeholder.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.loading
    }
}

/// Directional intents for host-managed tree keyboard traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeNavigation {
    /// Move to the previous visible node.
    Up,
    /// Move to the next visible node.
    Down,
    /// Move to the first visible node.
    Home,
    /// Move to the last visible node.
    End,
    /// Move toward the parent or collapse the current branch.
    Left,
    /// Move toward children or expand the current branch.
    Right,
}

/// A host-applied result from [`TreeView::navigation_outcome`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TreeNavigationOutcome {
    /// Select the provided visible node identifier.
    Select(SharedString),
    /// Toggle the provided branch node identifier.
    Toggle(SharedString),
    /// No action is required for the requested navigation intent.
    Noop,
}

/// Node selection behavior for [`TreeView`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TreeSelectionMode {
    /// Replace the current selection with one focused node.
    #[default]
    Single,
    /// Allow host-managed toggle and range selection.
    Multiple,
    /// Render checkbox affordances and emit host-managed toggle selection.
    Checkbox,
}

/// Selection intent emitted by pointer and keyboard interactions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeSelectionIntent {
    /// Replace the current selection with one node.
    Replace,
    /// Toggle one node in the current selection.
    Toggle,
    /// Select the range between the anchor node and focused node.
    Extend,
}

/// Host-applied node selection update for [`TreeView`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeSelection {
    intent: TreeSelectionIntent,
    anchor_id: SharedString,
    focused_id: SharedString,
    selected_ids: Vec<SharedString>,
}

impl TreeSelection {
    fn new(
        intent: TreeSelectionIntent,
        anchor_id: SharedString,
        focused_id: SharedString,
        selected_ids: Vec<SharedString>,
    ) -> Self {
        Self {
            intent,
            anchor_id,
            focused_id,
            selected_ids,
        }
    }

    /// Returns the originating selection intent.
    #[must_use]
    pub fn intent(&self) -> TreeSelectionIntent {
        self.intent
    }

    /// Returns the range anchor node identifier.
    #[must_use]
    pub fn anchor_id(&self) -> &SharedString {
        &self.anchor_id
    }

    /// Returns the focused node identifier.
    #[must_use]
    pub fn focused_id(&self) -> &SharedString {
        &self.focused_id
    }

    /// Returns selected node identifiers in visible tree order.
    #[must_use]
    pub fn selected_ids(&self) -> &[SharedString] {
        &self.selected_ids
    }
}

/// Immutable tree node metadata for [`TreeView`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeNode {
    id: SharedString,
    label: SharedString,
    detail: Option<SharedString>,
    children: Vec<TreeNode>,
    lazy_children: bool,
    expanded: bool,
    selected: bool,
    checked: bool,
    loading: bool,
}

impl TreeNode {
    /// Creates a new tree node.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            children: Vec::new(),
            lazy_children: false,
            expanded: false,
            selected: false,
            checked: false,
            loading: false,
        }
    }

    /// Attaches supporting detail text to the node row.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Replaces child nodes.
    #[must_use]
    pub fn children(mut self, children: Vec<TreeNode>) -> Self {
        self.children = children;
        self
    }

    /// Marks the node as a branch whose children can be loaded by the host.
    ///
    /// This is useful when a node should expose expansion and toggle behavior
    /// before its child collection is available.
    #[must_use]
    pub fn lazy_children(mut self, lazy_children: bool) -> Self {
        self.lazy_children = lazy_children;
        self
    }

    /// Sets the expansion state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets the selection state.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the checkbox state used by [`TreeSelectionMode::Checkbox`].
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Marks the node as awaiting lazy-loaded children.
    #[must_use]
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        if loading {
            self.lazy_children = true;
        }
        self
    }

    /// Returns the stable node identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns whether the node is currently expanded.
    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the child node slice.
    #[must_use]
    pub fn child_nodes(&self) -> &[TreeNode] {
        &self.children
    }

    /// Returns whether the node is a branch, including lazy-load branches.
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty() || self.lazy_children || self.loading
    }

    /// Returns whether the node is checked in checkbox selection mode.
    #[must_use]
    pub fn is_checked(&self) -> bool {
        self.checked
    }
}

/// A hierarchical tree surface for navigation and inspector-style views.
///
/// # Example
///
/// ```no_run
/// use guic_components::{TreeNode, TreeView};
///
/// let tree = TreeView::new("project-tree").nodes(vec![
///     TreeNode::new("src", "src").expanded(true).children(vec![
///         TreeNode::new("main", "main.rs").selected(true),
///     ]),
/// ]);
/// ```
#[derive(gpui::IntoElement)]
pub struct TreeView {
    id: SharedString,
    title: Option<SharedString>,
    nodes: Vec<TreeNode>,
    empty_label: SharedString,
    on_select: Option<SharedStringHandler>,
    on_node_selection: Option<TreeSelectionHandler>,
    on_toggle: Option<SharedStringHandler>,
    selection_mode: TreeSelectionMode,
    focus_handle: Option<FocusHandle>,
    row_height: f32,
    viewport: Option<TreeViewport>,
    max_height: Option<f32>,
}

impl TreeView {
    /// Creates a new tree view.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            nodes: Vec::new(),
            empty_label: "No items".into(),
            on_select: None,
            on_node_selection: None,
            on_toggle: None,
            selection_mode: TreeSelectionMode::Single,
            focus_handle: None,
            row_height: 36.0,
            viewport: None,
            max_height: None,
        }
    }

    /// Sets an optional section title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Replaces the root node collection.
    #[must_use]
    pub fn nodes(mut self, nodes: Vec<TreeNode>) -> Self {
        self.nodes = nodes;
        self
    }

    /// Overrides the empty-state label.
    #[must_use]
    pub fn empty_label(mut self, empty_label: impl Into<SharedString>) -> Self {
        self.empty_label = empty_label.into();
        self
    }

    /// Invokes a callback when a node row is selected.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Sets the node selection mode.
    #[must_use]
    pub fn selection_mode(mut self, selection_mode: TreeSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Invokes a callback when a node selection update is requested.
    ///
    /// This callback is the richer selection counterpart to [`Self::on_select`].
    /// It emits visible-tree-ordered selected node identifiers for replace,
    /// toggle, and range selection intents.
    #[must_use]
    pub fn on_node_selection(
        mut self,
        handler: impl Fn(&TreeSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_node_selection = Some(Rc::new(handler));
        self
    }

    /// Invokes a callback when a branch node marker is toggled.
    #[must_use]
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Makes the tree keyboard-focusable so that arrow keys and `Home`/`End`
    /// drive selection and branch expansion through [`Self::on_select`] and
    /// [`Self::on_toggle`].
    ///
    /// The host owns the [`FocusHandle`] (typically created once with
    /// `cx.focus_handle()`) so focus survives across re-renders. Keyboard
    /// navigation is wired up only when a focus handle is present.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Sets the expected row height used for virtualization math.
    #[must_use]
    pub fn row_height(mut self, row_height: f32) -> Self {
        if row_height.is_finite() {
            self.row_height = row_height.max(24.0);
        }
        self
    }

    /// Applies externally managed viewport metadata for node virtualization.
    #[must_use]
    pub fn viewport(mut self, viewport: TreeViewport) -> Self {
        self.viewport = Some(viewport);
        self
    }

    /// Caps the scrollable tree body height in logical pixels.
    #[must_use]
    pub fn max_height(mut self, max_height: f32) -> Self {
        if max_height.is_finite() {
            self.max_height = Some(max_height.max(96.0));
        }
        self
    }

    /// Applies a controlled update to an existing node.
    ///
    /// The returned tree contains the update. A missing target produces an
    /// error instead of silently dropping host state.
    pub fn apply_mutation(mut self, mut mutation: TreeMutation) -> Result<Self, TreeMutationError> {
        if apply_tree_mutation(&mut self.nodes, &mut mutation) {
            Ok(self)
        } else {
            Err(TreeMutationError {
                node_id: mutation.node_id,
            })
        }
    }

    /// Returns the visible node identifiers in depth-first render order.
    #[must_use]
    pub fn visible_node_ids(&self) -> Vec<SharedString> {
        self.visible_nodes()
            .into_iter()
            .map(|node| node.id)
            .collect()
    }

    /// Returns flattened visible node metadata in depth-first render order.
    #[must_use]
    pub fn visible_nodes(&self) -> Vec<VisibleTreeNode> {
        let mut nodes = Vec::new();
        for node in &self.nodes {
            collect_visible_nodes(node, 0, &mut nodes);
        }
        nodes
    }

    /// Returns the node metadata currently included in the render window.
    #[must_use]
    pub fn rendered_nodes(&self) -> Vec<VisibleTreeNode> {
        let visible = self.visible_nodes();
        let (start, end, _, _) = self.render_range(visible.len());
        visible
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect()
    }

    /// Returns the currently selected visible node identifier, if any.
    #[must_use]
    pub fn selected_node_id(&self) -> Option<SharedString> {
        self.visible_nodes()
            .into_iter()
            .find(|node| node.selected)
            .map(|node| node.id)
    }

    /// Returns selected visible node identifiers in depth-first render order.
    #[must_use]
    pub fn selected_node_ids(&self) -> Vec<SharedString> {
        self.visible_nodes()
            .into_iter()
            .filter(|node| node.selected)
            .map(|node| node.id)
            .collect()
    }

    /// Returns checked visible node identifiers in depth-first render order.
    #[must_use]
    pub fn checked_node_ids(&self) -> Vec<SharedString> {
        self.visible_nodes()
            .into_iter()
            .filter(|node| node.checked)
            .map(|node| node.id)
            .collect()
    }

    /// Returns the next visible node identifier after the current selection.
    #[must_use]
    pub fn next_visible_node_id(&self, current: &str) -> Option<SharedString> {
        adjacent_visible_node_id(self.visible_nodes(), current, 1)
    }

    /// Returns the previous visible node identifier before the current selection.
    #[must_use]
    pub fn previous_visible_node_id(&self, current: &str) -> Option<SharedString> {
        adjacent_visible_node_id(self.visible_nodes(), current, -1)
    }

    /// Returns the visible parent node identifier for the given node, if any.
    #[must_use]
    pub fn parent_node_id(&self, current: &str) -> Option<SharedString> {
        find_parent_node_id(&self.nodes, current)
    }

    /// Returns the host-applied outcome for a directional navigation intent.
    #[must_use]
    pub fn navigation_outcome(
        &self,
        current: &str,
        navigation: TreeNavigation,
    ) -> TreeNavigationOutcome {
        compute_tree_navigation(&self.visible_nodes(), current, navigation)
    }

    /// Returns visible node identifiers between two nodes, inclusive.
    #[must_use]
    pub fn node_range_ids(&self, anchor_id: &str, focused_id: &str) -> Vec<SharedString> {
        node_range_ids(&self.visible_node_ids(), anchor_id, focused_id)
    }

    /// Returns a host-applied selection update for a node interaction.
    #[must_use]
    pub fn selection_change(
        &self,
        node_id: &str,
        intent: TreeSelectionIntent,
    ) -> Option<TreeSelection> {
        let node_ids = self.visible_node_ids();
        let selected_ids = match self.selection_mode {
            TreeSelectionMode::Checkbox => self.checked_node_ids(),
            TreeSelectionMode::Single | TreeSelectionMode::Multiple => self.selected_node_ids(),
        };
        selection_change_for(
            &node_ids,
            &selected_ids,
            node_id,
            intent,
            self.selection_mode,
        )
    }

    fn render_range(&self, node_count: usize) -> (usize, usize, f32, f32) {
        let Some(viewport) = self.viewport else {
            return (0, node_count, 0.0, 0.0);
        };
        let metrics = VirtualListMetrics::new(
            self.row_height,
            viewport.viewport_height(),
            viewport.overscan_rows(),
            node_count,
        );
        let range = metrics.visible_range(viewport.scroll_offset());
        let start = range.start.min(node_count);
        let end = range.end.min(node_count);
        let top = start as f32 * self.row_height;
        let bottom = (metrics.total_height() - end as f32 * self.row_height).max(0.0);
        (start, end, top, bottom)
    }
}

fn compute_tree_navigation(
    visible_nodes: &[VisibleTreeNode],
    current: &str,
    navigation: TreeNavigation,
) -> TreeNavigationOutcome {
    let Some(index) = visible_nodes
        .iter()
        .position(|node| node.id.as_ref() == current)
    else {
        return TreeNavigationOutcome::Noop;
    };
    let current_node = &visible_nodes[index];

    match navigation {
        TreeNavigation::Up => visible_nodes
            .get(index.saturating_sub(1))
            .map_or(TreeNavigationOutcome::Noop, |node| {
                TreeNavigationOutcome::Select(node.id.clone())
            }),
        TreeNavigation::Down => visible_nodes
            .get(index.saturating_add(1))
            .map_or(TreeNavigationOutcome::Noop, |node| {
                TreeNavigationOutcome::Select(node.id.clone())
            }),
        TreeNavigation::Home => visible_nodes
            .first()
            .map_or(TreeNavigationOutcome::Noop, |node| {
                TreeNavigationOutcome::Select(node.id.clone())
            }),
        TreeNavigation::End => visible_nodes
            .last()
            .map_or(TreeNavigationOutcome::Noop, |node| {
                TreeNavigationOutcome::Select(node.id.clone())
            }),
        TreeNavigation::Left => {
            if current_node.has_children && current_node.expanded {
                TreeNavigationOutcome::Toggle(current_node.id.clone())
            } else {
                visible_parent_node_id(visible_nodes, index)
                    .map_or(TreeNavigationOutcome::Noop, TreeNavigationOutcome::Select)
            }
        }
        TreeNavigation::Right => {
            if current_node.has_children && !current_node.expanded {
                TreeNavigationOutcome::Toggle(current_node.id.clone())
            } else {
                visible_nodes
                    .get(index.saturating_add(1))
                    .filter(|node| node.depth == current_node.depth.saturating_add(1))
                    .map_or(TreeNavigationOutcome::Noop, |node| {
                        TreeNavigationOutcome::Select(node.id.clone())
                    })
            }
        }
    }
}

fn tree_navigation_for(keystroke: &Keystroke) -> Option<TreeNavigation> {
    match keystroke.key.as_str() {
        "up" => Some(TreeNavigation::Up),
        "down" => Some(TreeNavigation::Down),
        "home" => Some(TreeNavigation::Home),
        "end" => Some(TreeNavigation::End),
        "left" => Some(TreeNavigation::Left),
        "right" => Some(TreeNavigation::Right),
        _ => None,
    }
}

impl RenderOnce for TreeView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx).clone();
        let focus_handle = self.focus_handle.clone();
        let visible_nodes = self.visible_nodes();
        let visible_node_ids = visible_nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let selected_node_id = visible_nodes
            .iter()
            .find(|node| node.selected)
            .map(|node| node.id.clone());
        let selected_node_ids = match self.selection_mode {
            TreeSelectionMode::Checkbox => visible_nodes
                .iter()
                .filter(|node| node.checked)
                .map(|node| node.id.clone())
                .collect::<Vec<_>>(),
            TreeSelectionMode::Single | TreeSelectionMode::Multiple => visible_nodes
                .iter()
                .filter(|node| node.selected)
                .map(|node| node.id.clone())
                .collect::<Vec<_>>(),
        };
        let shared_visible_node_ids = Rc::new(visible_node_ids.clone());
        let shared_selected_node_ids = Rc::new(selected_node_ids.clone());
        let (render_start, render_end, top_spacer, bottom_spacer) =
            self.render_range(visible_nodes.len());
        let keyboard = focus_handle.as_ref().map(|_| {
            (
                visible_nodes.clone(),
                visible_node_ids.clone(),
                selected_node_ids.clone(),
                selected_node_id.clone(),
                self.selection_mode,
                self.on_select.clone(),
                self.on_node_selection.clone(),
                self.on_toggle.clone(),
            )
        });
        let mut root = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Tree)
                    .label(self.title.clone().unwrap_or_else(|| self.id.clone())),
            )
            .w_full()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .p_3()
            .flex()
            .flex_col()
            .gap_2();

        if let Some(handle) = focus_handle {
            root = root.key_context("GuicTreeView").track_focus(&handle);
        }

        if let Some((
            visible,
            visible_ids,
            selected_ids,
            selected,
            selection_mode,
            on_select,
            on_node_selection,
            on_toggle,
        )) = keyboard
        {
            root = root.on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "space" {
                    let Some(current) = selected.clone().or_else(|| {
                        visible
                            .first()
                            .map(|node: &VisibleTreeNode| node.id().clone())
                    }) else {
                        return;
                    };
                    emit_tree_selection(
                        &current,
                        TreeSelectionIntent::Toggle,
                        &visible_ids,
                        &selected_ids,
                        selection_mode,
                        on_select.as_ref(),
                        on_node_selection.as_ref(),
                        window,
                        cx,
                    );
                    return;
                }

                let Some(navigation) = tree_navigation_for(&event.keystroke) else {
                    return;
                };
                let current = match &selected {
                    Some(current) => current.clone(),
                    None => match visible.first() {
                        Some(node) => node.id().clone(),
                        None => return,
                    },
                };
                match compute_tree_navigation(&visible, current.as_ref(), navigation) {
                    TreeNavigationOutcome::Select(id) => {
                        let intent = if event.keystroke.modifiers.shift {
                            TreeSelectionIntent::Extend
                        } else {
                            TreeSelectionIntent::Replace
                        };
                        emit_tree_selection(
                            &id,
                            intent,
                            &visible_ids,
                            &selected_ids,
                            selection_mode,
                            on_select.as_ref(),
                            on_node_selection.as_ref(),
                            window,
                            cx,
                        );
                    }
                    TreeNavigationOutcome::Toggle(id) => {
                        if let Some(handler) = &on_toggle {
                            handler(&id, window, cx);
                        }
                    }
                    TreeNavigationOutcome::Noop => {}
                }
            });
        }

        if let Some(title) = &self.title {
            root = root.child(Label::new(title.clone()).muted(true));
        }

        if self.nodes.is_empty() {
            return root
                .child(Label::new(self.empty_label.clone()).muted(true))
                .into_any_element();
        }

        let mut visible_node_refs = Vec::with_capacity(visible_nodes.len());
        for node in &self.nodes {
            collect_visible_node_refs(node, 0, &mut visible_node_refs);
        }
        let context = TreeRenderContext {
            on_select: self.on_select.clone(),
            on_node_selection: self.on_node_selection.clone(),
            on_toggle: self.on_toggle.clone(),
            selection_mode: self.selection_mode,
            visible_node_ids: shared_visible_node_ids,
            selected_node_ids: shared_selected_node_ids,
            theme: theme.clone(),
        };
        let mut rows = div().w_full().flex().flex_col().gap_1();
        if top_spacer > 0.0 {
            rows = rows.child(div().w_full().h(px(top_spacer)));
        }
        for (node, depth) in visible_node_refs
            .into_iter()
            .skip(render_start)
            .take(render_end.saturating_sub(render_start))
        {
            rows = rows.child(render_node_row(node, depth, context.clone()));
        }
        if bottom_spacer > 0.0 {
            rows = rows.child(div().w_full().h(px(bottom_spacer)));
        }

        let body = if let Some(max_height) = self.max_height {
            div().w_full().h(px(max_height)).child(
                ScrollArea::new("guic-tree-scroll", rows)
                    .vertical(true)
                    .horizontal(false),
            )
        } else {
            div().w_full().child(rows)
        };
        root.child(body).into_any_element()
    }
}

fn render_node_row(node: &TreeNode, depth: usize, context: TreeRenderContext) -> AnyElement {
    let TreeRenderContext {
        on_select,
        on_node_selection,
        on_toggle,
        selection_mode,
        visible_node_ids,
        selected_node_ids,
        theme,
    } = context;
    let has_children = node.has_children();
    let marker = if node.loading {
        "..."
    } else if has_children && node.expanded {
        "v"
    } else if has_children {
        ">"
    } else {
        "-"
    };

    let node_id = node.id.clone();
    let mut accessibility = AccessibilityProps::new(Role::TreeItem)
        .label(node.label.clone())
        .selected(node.selected);
    if has_children {
        accessibility = accessibility.expanded(node.expanded);
    }
    if selection_mode == TreeSelectionMode::Checkbox {
        accessibility = accessibility.checked(node.checked);
    }
    let mut row = div()
        .id(node.id.clone())
        .accessibility(accessibility)
        .debug_selector({
            let node_id = node.id.clone();
            move || format!("guic-tree-row-{node_id}")
        })
        .w_full()
        .pl(px(depth as f32 * 16.0))
        .pr_2()
        .py_2()
        .rounded(px(theme.radius.md))
        .flex()
        .items_center()
        .gap_2()
        .bg(if node.selected {
            theme.primary().opacity(0.12)
        } else {
            theme.background().opacity(0.0)
        })
        .child({
            let marker_cell = div()
                .w(px(20.0))
                .text_color(theme.muted_foreground())
                .child(marker);
            if has_children {
                if let Some(handler) = on_toggle.clone() {
                    marker_cell
                        .id(format!("guic-tree-toggle-{}", node.id))
                        .debug_selector({
                            let node_id = node.id.clone();
                            move || format!("guic-tree-toggle-{node_id}")
                        })
                        .cursor_pointer()
                        .on_click(move |_, window, cx| handler(&node_id, window, cx))
                        .into_any_element()
                } else {
                    marker_cell.into_any_element()
                }
            } else {
                marker_cell.into_any_element()
            }
        })
        .children({
            let mut children = Vec::new();
            if selection_mode == TreeSelectionMode::Checkbox {
                let checkbox_id = node.id.clone();
                let checkbox_label = if node.checked { "[x]" } else { "[ ]" };
                let mut checkbox = div()
                    .id(format!("guic-tree-checkbox-{}", node.id))
                    .debug_selector({
                        let node_id = node.id.clone();
                        move || format!("guic-tree-checkbox-{node_id}")
                    })
                    .text_color(theme.muted_foreground())
                    .child(checkbox_label);
                if on_node_selection.is_some() || on_select.is_some() {
                    let row_handler = on_select.clone();
                    let selection_handler = on_node_selection.clone();
                    let visible_ids = visible_node_ids.clone();
                    let selected_ids = selected_node_ids.clone();
                    checkbox = checkbox.cursor_pointer().on_click(move |_, window, cx| {
                        emit_tree_selection(
                            &checkbox_id,
                            TreeSelectionIntent::Toggle,
                            &visible_ids,
                            &selected_ids,
                            selection_mode,
                            row_handler.as_ref(),
                            selection_handler.as_ref(),
                            window,
                            cx,
                        );
                    });
                }
                children.push(checkbox.into_any_element());
            }
            children.push(Label::new(node.label.clone()).into_any_element());
            children
        });

    if let Some(detail) = &node.detail {
        row = row.child(Label::new(detail.clone()).muted(true));
    } else if node.loading && node.expanded {
        row = row.child(Label::new("Loading children...").muted(true));
    }

    if on_select.is_some() || on_node_selection.is_some() {
        let node_id = node.id.clone();
        let row_handler = on_select.clone();
        let selection_handler = on_node_selection.clone();
        let visible_ids = visible_node_ids.clone();
        let selected_ids = selected_node_ids.clone();
        row = row.cursor_pointer().on_click(move |event, window, cx| {
            let modifiers = event.modifiers();
            let intent = if modifiers.shift {
                TreeSelectionIntent::Extend
            } else if modifiers.platform
                || modifiers.control
                || selection_mode == TreeSelectionMode::Checkbox
            {
                TreeSelectionIntent::Toggle
            } else {
                TreeSelectionIntent::Replace
            };
            emit_tree_selection(
                &node_id,
                intent,
                &visible_ids,
                &selected_ids,
                selection_mode,
                row_handler.as_ref(),
                selection_handler.as_ref(),
                window,
                cx,
            );
        });
    }

    row.into_any_element()
}

#[derive(Clone)]
struct TreeRenderContext {
    on_select: Option<SharedStringHandler>,
    on_node_selection: Option<TreeSelectionHandler>,
    on_toggle: Option<SharedStringHandler>,
    selection_mode: TreeSelectionMode,
    visible_node_ids: Rc<Vec<SharedString>>,
    selected_node_ids: Rc<Vec<SharedString>>,
    theme: Theme,
}

fn collect_visible_nodes(node: &TreeNode, depth: usize, nodes: &mut Vec<VisibleTreeNode>) {
    nodes.push(VisibleTreeNode::new(node, depth));
    if node.expanded {
        for child in &node.children {
            collect_visible_nodes(child, depth + 1, nodes);
        }
    }
}

fn collect_visible_node_refs<'a>(
    node: &'a TreeNode,
    depth: usize,
    nodes: &mut Vec<(&'a TreeNode, usize)>,
) {
    nodes.push((node, depth));
    if node.expanded {
        for child in &node.children {
            collect_visible_node_refs(child, depth + 1, nodes);
        }
    }
}

fn visible_parent_node_id(
    visible_nodes: &[VisibleTreeNode],
    current_index: usize,
) -> Option<SharedString> {
    let parent_depth = visible_nodes.get(current_index)?.depth.checked_sub(1)?;
    visible_nodes[..current_index]
        .iter()
        .rev()
        .find(|node| node.depth == parent_depth)
        .map(|node| node.id.clone())
}

fn apply_tree_mutation(nodes: &mut [TreeNode], mutation: &mut TreeMutation) -> bool {
    for node in nodes {
        if node.id == mutation.node_id {
            match &mut mutation.kind {
                TreeMutationKind::Expanded(expanded) => node.expanded = *expanded,
                TreeMutationKind::Selected(selected) => node.selected = *selected,
                TreeMutationKind::Checked(checked) => node.checked = *checked,
                TreeMutationKind::Loading(loading) => {
                    node.loading = *loading;
                    if *loading {
                        node.lazy_children = true;
                    }
                }
                TreeMutationKind::ReplaceChildren(children) => {
                    node.children = std::mem::take(children);
                    node.loading = false;
                    node.lazy_children = false;
                }
            }
            return true;
        }
        if apply_tree_mutation(&mut node.children, mutation) {
            return true;
        }
    }
    false
}

fn adjacent_visible_node_id(
    nodes: Vec<VisibleTreeNode>,
    current: &str,
    direction: isize,
) -> Option<SharedString> {
    let index = nodes.iter().position(|node| node.id.as_ref() == current)?;
    let next_index = if direction.is_negative() {
        index.checked_sub(direction.unsigned_abs())?
    } else {
        index.checked_add(direction as usize)?
    };
    nodes.get(next_index).map(|node| node.id.clone())
}

fn node_range_ids(ids: &[SharedString], anchor_id: &str, focused_id: &str) -> Vec<SharedString> {
    let Some(anchor_index) = ids.iter().position(|id| id.as_ref() == anchor_id) else {
        return Vec::new();
    };
    let Some(focused_index) = ids.iter().position(|id| id.as_ref() == focused_id) else {
        return Vec::new();
    };
    let start = anchor_index.min(focused_index);
    let end = anchor_index.max(focused_index);
    ids[start..=end].to_vec()
}

fn selection_change_for(
    node_ids: &[SharedString],
    selected_ids: &[SharedString],
    node_id: &str,
    intent: TreeSelectionIntent,
    selection_mode: TreeSelectionMode,
) -> Option<TreeSelection> {
    let focused_id = node_ids.iter().find(|id| id.as_ref() == node_id)?.clone();
    let anchor_id = selected_ids
        .first()
        .cloned()
        .unwrap_or_else(|| focused_id.clone());
    let effective_intent = match selection_mode {
        TreeSelectionMode::Single => TreeSelectionIntent::Replace,
        TreeSelectionMode::Multiple | TreeSelectionMode::Checkbox => intent,
    };
    let selected_ids = match effective_intent {
        TreeSelectionIntent::Replace => vec![focused_id.clone()],
        TreeSelectionIntent::Toggle => toggle_selection(node_ids, selected_ids, node_id),
        TreeSelectionIntent::Extend => node_range_ids(node_ids, anchor_id.as_ref(), node_id),
    };
    let selected_ids = if selected_ids.is_empty() && effective_intent != TreeSelectionIntent::Toggle
    {
        vec![focused_id.clone()]
    } else {
        selected_ids
    };

    Some(TreeSelection::new(
        effective_intent,
        anchor_id,
        focused_id,
        selected_ids,
    ))
}

fn toggle_selection(
    node_ids: &[SharedString],
    selected_ids: &[SharedString],
    node_id: &str,
) -> Vec<SharedString> {
    let was_selected = selected_ids.iter().any(|id| id.as_ref() == node_id);
    node_ids
        .iter()
        .filter(|id| {
            if id.as_ref() == node_id {
                !was_selected
            } else {
                selected_ids.iter().any(|selected| selected == *id)
            }
        })
        .cloned()
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn emit_tree_selection(
    node_id: &SharedString,
    intent: TreeSelectionIntent,
    node_ids: &[SharedString],
    selected_ids: &[SharedString],
    selection_mode: TreeSelectionMode,
    node_handler: Option<&SharedStringHandler>,
    selection_handler: Option<&TreeSelectionHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = selection_handler
        && let Some(selection) = selection_change_for(
            node_ids,
            selected_ids,
            node_id.as_ref(),
            intent,
            selection_mode,
        )
    {
        handler(&selection, window, cx);
        return;
    }

    if let Some(handler) = node_handler {
        handler(node_id, window, cx);
    }
}

fn find_parent_node_id(nodes: &[TreeNode], current: &str) -> Option<SharedString> {
    for node in nodes {
        if node
            .children
            .iter()
            .any(|child| child.id.as_ref() == current)
        {
            return Some(node.id.clone());
        }

        if let Some(parent) = find_parent_node_id(&node.children, current) {
            return Some(parent);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        TreeMutation, TreeNavigation, TreeNavigationOutcome, TreeNode, TreeSelection,
        TreeSelectionIntent, TreeSelectionMode, TreeView, TreeViewport,
    };
    use gpui::{
        AppContext as _, Context, FocusHandle, Keystroke, Modifiers, ParentElement as _, Render,
        SharedString, Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    struct TreeHarness {
        selected_node: String,
        root_expanded: bool,
        focus_handle: FocusHandle,
    }

    struct TreeSelectionHarness {
        selected_nodes: Vec<String>,
        focus_handle: FocusHandle,
    }

    struct VirtualTreeHarness;

    impl Render for VirtualTreeHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let nodes = (0..1_000)
                .map(|index| TreeNode::new(format!("virtual-{index}"), index.to_string()))
                .collect();
            div().size_full().child(
                TreeView::new("virtual-tree-test")
                    .nodes(nodes)
                    .row_height(32.0)
                    .viewport(TreeViewport::new(16_000.0, 160.0).overscan(1))
                    .max_height(160.0),
            )
        }
    }

    impl TreeHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                selected_node: "child".to_owned(),
                root_expanded: true,
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl TreeSelectionHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                selected_nodes: vec!["child-a".to_owned()],
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl Render for TreeSelectionHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let selected = |id: &str, nodes: &[String]| nodes.iter().any(|node| node == id);
            div().size_full().p_4().child(
                TreeView::new("selection-tree-test")
                    .focusable(self.focus_handle.clone())
                    .selection_mode(TreeSelectionMode::Checkbox)
                    .nodes(vec![
                        TreeNode::new("root", "root")
                            .expanded(true)
                            .checked(selected("root", &self.selected_nodes))
                            .children(vec![
                                TreeNode::new("child-a", "child-a")
                                    .checked(selected("child-a", &self.selected_nodes))
                                    .selected(selected("child-a", &self.selected_nodes)),
                                TreeNode::new("child-b", "child-b")
                                    .checked(selected("child-b", &self.selected_nodes))
                                    .selected(selected("child-b", &self.selected_nodes)),
                            ]),
                    ])
                    .on_node_selection(cx.listener(|this, selection: &TreeSelection, _, cx| {
                        this.selected_nodes = selection
                            .selected_ids()
                            .iter()
                            .map(ToString::to_string)
                            .collect();
                        cx.notify();
                    })),
            )
        }
    }

    fn selection_ids(selection: &TreeSelection) -> Vec<String> {
        selection
            .selected_ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    impl Render for TreeHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                TreeView::new("advanced-tree-test")
                    .focusable(self.focus_handle.clone())
                    .nodes(vec![
                        TreeNode::new("root", "root")
                            .expanded(self.root_expanded)
                            .selected(self.selected_node == "root")
                            .children(vec![
                                TreeNode::new("child", "child")
                                    .selected(self.selected_node == "child"),
                            ]),
                    ])
                    .on_select(cx.listener(|this, node_id: &SharedString, _, cx| {
                        this.selected_node = node_id.to_string();
                        cx.notify();
                    }))
                    .on_toggle(cx.listener(|this, node_id: &SharedString, _, cx| {
                        if node_id.as_ref() == "root" {
                            this.root_expanded = !this.root_expanded;
                            cx.notify();
                        }
                    })),
            )
        }
    }

    #[test]
    fn tree_node_tracks_hierarchy() {
        let node = TreeNode::new("root", "Root")
            .expanded(true)
            .children(vec![TreeNode::new("leaf", "Leaf")]);
        assert!(node.expanded);
        assert_eq!(node.children.len(), 1);
    }

    #[test]
    fn non_finite_tree_layout_configuration_is_bounded() {
        let viewport = TreeViewport::new(f32::INFINITY, f32::NAN);
        assert_eq!(viewport.scroll_offset(), 0.0);
        assert_eq!(viewport.viewport_height(), 0.0);

        let tree = TreeView::new("tree")
            .row_height(f32::INFINITY)
            .max_height(f32::NAN);
        assert_eq!(tree.row_height, 36.0);
        assert_eq!(tree.max_height, None);
    }

    #[test]
    fn tree_view_accepts_custom_empty_label() {
        let tree = TreeView::new("empty").empty_label("Nothing to show");
        assert_eq!(tree.empty_label, "Nothing to show");
    }

    #[test]
    fn tree_node_selection_builder_sets_flag() {
        let node = TreeNode::new("leaf", "Leaf").selected(true);
        assert!(node.selected);
    }

    #[test]
    fn tree_node_checkbox_builder_sets_flag() {
        let node = TreeNode::new("leaf", "Leaf").checked(true);
        assert!(node.is_checked());
    }

    #[test]
    fn tree_node_can_represent_lazy_branches_without_children() {
        let lazy = TreeNode::new("remote", "Remote").lazy_children(true);
        let loading = TreeNode::new("loading", "Loading").loading(true);
        let leaf = TreeNode::new("leaf", "Leaf");

        assert!(lazy.has_children());
        assert!(loading.has_children());
        assert!(!leaf.has_children());
    }

    #[test]
    fn tree_view_reports_visible_node_order() {
        let tree = TreeView::new("project-tree").nodes(vec![
            TreeNode::new("src", "src")
                .expanded(true)
                .children(vec![TreeNode::new("main", "main.rs")]),
            TreeNode::new("tests", "tests"),
        ]);

        let ids = tree.visible_node_ids();
        assert_eq!(ids, vec!["src", "main", "tests"]);
    }

    #[test]
    fn tree_view_reports_visible_node_metadata() {
        let tree = TreeView::new("project-tree").nodes(vec![
            TreeNode::new("src", "src")
                .expanded(true)
                .children(vec![TreeNode::new("main", "main.rs").selected(true)]),
        ]);

        let visible = tree.visible_nodes();
        assert_eq!(visible[0].depth(), 0);
        assert!(visible[0].has_children());
        assert_eq!(visible[1].depth(), 1);
        assert!(visible[1].is_selected());
    }

    #[test]
    fn tree_view_virtualizes_large_visible_node_sets() {
        let nodes = (0..100_000)
            .map(|index| TreeNode::new(format!("node-{index}"), index.to_string()))
            .collect::<Vec<_>>();
        let tree = TreeView::new("large-tree")
            .nodes(nodes)
            .row_height(32.0)
            .viewport(TreeViewport::new(1_600_000.0, 320.0).overscan(2));

        let rendered = tree.rendered_nodes();
        assert_eq!(rendered.len(), 14);
        assert_eq!(rendered[0].id().as_ref(), "node-49998");
        assert_eq!(rendered[13].id().as_ref(), "node-50011");
    }

    #[gpui::test]
    fn tree_virtualized_render_omits_nodes_outside_window(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (_view, cx) = cx.add_window_view(|_, _| VirtualTreeHarness);

        assert!(cx.debug_bounds("guic-tree-row-virtual-499").is_some());
        assert!(cx.debug_bounds("guic-tree-row-virtual-0").is_none());
        assert!(cx.debug_bounds("guic-tree-row-virtual-999").is_none());
    }

    #[test]
    fn tree_mutations_update_nested_controlled_state() {
        let tree = TreeView::new("mutations").nodes(vec![
            TreeNode::new("root", "Root")
                .lazy_children(true)
                .children(vec![TreeNode::new("child", "Child")]),
        ]);
        let tree = tree
            .apply_mutation(TreeMutation::expanded("root", true))
            .expect("root exists")
            .apply_mutation(TreeMutation::selected("child", true))
            .expect("child exists")
            .apply_mutation(TreeMutation::checked("child", true))
            .expect("child exists");

        let visible = tree.visible_nodes();
        assert_eq!(visible.len(), 2);
        assert!(visible[0].is_expanded());
        assert!(visible[1].is_selected());
        assert!(visible[1].is_checked());
    }

    #[test]
    fn tree_mutation_replaces_lazy_children_and_reports_missing_targets() {
        let tree = TreeView::new("lazy").nodes(vec![
            TreeNode::new("remote", "Remote")
                .expanded(true)
                .loading(true),
        ]);
        let tree = tree
            .apply_mutation(TreeMutation::replace_children(
                "remote",
                vec![TreeNode::new("loaded", "Loaded")],
            ))
            .expect("remote exists");
        assert_eq!(tree.visible_node_ids(), vec!["remote", "loaded"]);

        let error = match tree.apply_mutation(TreeMutation::loading("missing", true)) {
            Ok(_) => panic!("missing nodes must not be ignored"),
            Err(error) => error,
        };
        assert_eq!(error.node_id().as_ref(), "missing");
    }

    #[test]
    fn tree_view_reports_checked_node_metadata() {
        let tree = TreeView::new("project-tree")
            .selection_mode(TreeSelectionMode::Checkbox)
            .nodes(vec![TreeNode::new("src", "src").checked(true)]);

        let visible = tree.visible_nodes();
        assert!(visible[0].is_checked());
        assert_eq!(tree.checked_node_ids(), vec!["src"]);
    }

    #[test]
    fn tree_view_reports_lazy_branch_metadata() {
        let tree = TreeView::new("project-tree").nodes(vec![
            TreeNode::new("remote", "Remote")
                .lazy_children(true)
                .selected(true),
        ]);

        let visible = tree.visible_nodes();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].has_children());
        assert!(!visible[0].is_loading());
    }

    #[test]
    fn tree_view_computes_adjacent_visible_nodes() {
        let tree = TreeView::new("workspace").nodes(vec![
            TreeNode::new("root", "root")
                .expanded(true)
                .children(vec![TreeNode::new("child", "child")]),
        ]);

        assert_eq!(tree.next_visible_node_id("root").as_deref(), Some("child"));
        assert_eq!(
            tree.previous_visible_node_id("child").as_deref(),
            Some("root")
        );
        assert_eq!(tree.next_visible_node_id("child"), None);
    }

    #[test]
    fn tree_view_reports_selected_and_parent_nodes() {
        let tree = TreeView::new("workspace").nodes(vec![
            TreeNode::new("root", "root").expanded(true).children(vec![
                TreeNode::new("child", "child")
                    .expanded(true)
                    .children(vec![TreeNode::new("leaf", "leaf").selected(true)]),
            ]),
        ]);

        assert_eq!(tree.selected_node_id().as_deref(), Some("leaf"));
        assert_eq!(tree.parent_node_id("leaf").as_deref(), Some("child"));
        assert_eq!(tree.parent_node_id("root"), None);
    }

    #[test]
    fn tree_view_produces_keyboard_navigation_outcomes() {
        let tree = TreeView::new("workspace").nodes(vec![
            TreeNode::new("root", "root").expanded(true).children(vec![
                TreeNode::new("collapsed", "collapsed")
                    .children(vec![TreeNode::new("inner", "inner")]),
                TreeNode::new("expanded", "expanded")
                    .expanded(true)
                    .children(vec![TreeNode::new("leaf", "leaf")]),
            ]),
        ]);

        assert_eq!(
            tree.navigation_outcome("root", TreeNavigation::Down),
            TreeNavigationOutcome::Select("collapsed".into())
        );
        assert_eq!(
            tree.navigation_outcome("collapsed", TreeNavigation::Right),
            TreeNavigationOutcome::Toggle("collapsed".into())
        );
        assert_eq!(
            tree.navigation_outcome("expanded", TreeNavigation::Right),
            TreeNavigationOutcome::Select("leaf".into())
        );
        assert_eq!(
            tree.navigation_outcome("expanded", TreeNavigation::Left),
            TreeNavigationOutcome::Toggle("expanded".into())
        );
        assert_eq!(
            tree.navigation_outcome("leaf", TreeNavigation::Left),
            TreeNavigationOutcome::Select("expanded".into())
        );
        assert_eq!(
            tree.navigation_outcome("root", TreeNavigation::Home),
            TreeNavigationOutcome::Select("root".into())
        );
        assert_eq!(
            tree.navigation_outcome("leaf", TreeNavigation::End),
            TreeNavigationOutcome::Select("leaf".into())
        );
    }

    #[test]
    fn tree_view_toggles_lazy_branches_from_keyboard_navigation() {
        let tree = TreeView::new("workspace").nodes(vec![
            TreeNode::new("remote", "Remote").lazy_children(true),
            TreeNode::new("loading", "Loading").loading(true),
        ]);

        assert_eq!(
            tree.navigation_outcome("remote", TreeNavigation::Right),
            TreeNavigationOutcome::Toggle("remote".into())
        );
        assert_eq!(
            tree.navigation_outcome("loading", TreeNavigation::Right),
            TreeNavigationOutcome::Toggle("loading".into())
        );
    }

    #[test]
    fn tree_view_produces_checkbox_selection_changes() {
        let tree = TreeView::new("workspace")
            .selection_mode(TreeSelectionMode::Checkbox)
            .nodes(vec![TreeNode::new("root", "root").expanded(true).children(
                vec![
                    TreeNode::new("child-a", "child-a").checked(true),
                    TreeNode::new("child-b", "child-b"),
                ],
            )]);

        let toggled = tree
            .selection_change("child-b", TreeSelectionIntent::Toggle)
            .expect("selection should exist");
        assert_eq!(toggled.intent(), TreeSelectionIntent::Toggle);
        assert_eq!(selection_ids(&toggled), vec!["child-a", "child-b"]);

        let range = tree
            .selection_change("root", TreeSelectionIntent::Extend)
            .expect("selection should exist");
        assert_eq!(selection_ids(&range), vec!["root", "child-a"]);
    }

    #[test]
    fn single_tree_selection_coerces_toggle_to_replace() {
        let tree = TreeView::new("workspace").nodes(vec![
            TreeNode::new("root", "root")
                .expanded(true)
                .selected(true)
                .children(vec![TreeNode::new("child", "child")]),
        ]);

        let selection = tree
            .selection_change("child", TreeSelectionIntent::Toggle)
            .expect("selection should exist");
        assert_eq!(selection.intent(), TreeSelectionIntent::Replace);
        assert_eq!(selection_ids(&selection), vec!["child"]);
    }

    #[gpui::test]
    fn tree_selection_and_toggle_handle_clicks(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| TreeHarness::new(cx));

        let root_bounds = cx
            .debug_bounds("guic-tree-row-root")
            .expect("root row should exist");
        cx.simulate_click(root_bounds.center(), Modifiers::none());

        let toggle_bounds = cx
            .debug_bounds("guic-tree-toggle-root")
            .expect("root toggle should exist");
        cx.simulate_click(toggle_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.selected_node, "root");
            assert!(!view.root_expanded);
        });
    }

    #[gpui::test]
    fn tree_keyboard_navigation_selects_and_toggles(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| TreeHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        // Selection starts on "child"; Up moves to the parent "root".
        cx.dispatch_keystroke(window, Keystroke::parse("up").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.selected_node, "root");
            assert!(view.root_expanded);
        });

        // Left on an expanded branch collapses it through on_toggle.
        cx.dispatch_keystroke(window, Keystroke::parse("left").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.selected_node, "root");
            assert!(!view.root_expanded);
        });
    }

    #[gpui::test]
    fn tree_checkbox_click_toggles_node_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| TreeSelectionHarness::new(cx));

        let checkbox_bounds = cx
            .debug_bounds("guic-tree-checkbox-child-b")
            .expect("tree checkbox bounds should exist");
        cx.simulate_click(checkbox_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.selected_nodes, vec!["child-a", "child-b"]);
        });
    }

    #[gpui::test]
    fn tree_space_toggles_current_checkbox_node(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| TreeSelectionHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("space").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert!(view.selected_nodes.is_empty());
        });
    }
}
