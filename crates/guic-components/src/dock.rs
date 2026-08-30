use crate::{Badge, BadgeVariant, Label, Separator};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Render, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
    px, relative,
};
use guic_tokens::Theme;
use serde::{Deserialize, Serialize};
use std::{cell::Cell, collections::HashMap, rc::Rc};

type DockCommandHandler = Rc<dyn Fn(&DockCommand, &mut Window, &mut App)>;
type DockTabBodyRenderer = Rc<dyn Fn(&DockTabSelection, &DockTab) -> AnyElement>;

#[derive(Clone, Default)]
struct DockRenderHandlers {
    on_command: Option<DockCommandHandler>,
    tab_body_renderer: Option<DockTabBodyRenderer>,
    tab_scroll_handles: Rc<HashMap<SharedString, ScrollHandle>>,
    focus_handle: Option<FocusHandle>,
    focused_stack_id: Option<SharedString>,
}

/// Split axis metadata for [`DockNode::Split`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockAxis {
    /// Lay out child nodes from left to right.
    Horizontal,
    /// Lay out child nodes from top to bottom.
    Vertical,
}

/// Placement metadata for splitting a dock stack at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockPlacement {
    /// Insert a new stack to the left of the target.
    Left,
    /// Insert a new stack to the right of the target.
    Right,
    /// Insert a new stack above the target.
    Top,
    /// Insert a new stack below the target.
    Bottom,
}

/// Drop zone exposed by a dock stack during pointer docking.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockDropZone {
    /// Insert the tab into the target stack.
    Center,
    /// Split to the left of the target stack.
    Left,
    /// Split to the right of the target stack.
    Right,
    /// Split above the target stack.
    Top,
    /// Split below the target stack.
    Bottom,
}

/// Tab identity carried through a pointer or command-driven docking operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockDragPayload {
    /// Source stack identifier.
    pub stack_id: SharedString,
    /// Dragged tab identifier.
    pub tab_id: SharedString,
}

impl Render for DockDragPayload {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        div()
            .px_3()
            .py_2()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .child(format!("{} / {}", self.stack_id, self.tab_id))
    }
}

/// Destination for a tab docking operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockDropTarget {
    /// Existing destination stack.
    pub stack_id: SharedString,
    /// Target drop zone.
    pub zone: DockDropZone,
    /// Identifier assigned to a stack created by an edge drop.
    pub new_stack_id: SharedString,
}

/// Complete command set for pointer and keyboard dock operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DockCommand {
    /// Select a tab.
    SelectTab(DockTabSelection),
    /// Select the previous or next tab in a stack.
    SelectAdjacentTab {
        /// Target stack.
        stack_id: SharedString,
        /// Negative selects previous; zero or positive selects next.
        direction: i8,
    },
    /// Move a tab by a signed number of positions in its current stack.
    MoveTab {
        /// Source stack.
        stack_id: SharedString,
        /// Tab to move.
        tab_id: SharedString,
        /// Signed position delta.
        delta: isize,
    },
    /// Focus the previous or next stack in layout order.
    FocusAdjacentStack {
        /// Stack from which traversal starts.
        stack_id: SharedString,
        /// Negative focuses previous; zero or positive focuses next.
        direction: i8,
    },
    /// Move a tab to the previous or next stack in layout order.
    MoveTabToAdjacentStack {
        /// Source stack.
        stack_id: SharedString,
        /// Tab to move.
        tab_id: SharedString,
        /// Negative moves to previous; zero or positive moves to next.
        direction: i8,
    },
    /// Split a tab out of its current stack.
    SplitTab {
        /// Tab to split out.
        selection: DockTabSelection,
        /// Side on which the new stack is created.
        placement: DockPlacement,
        /// Preferred identifier for the new stack.
        new_stack_id: SharedString,
    },
    /// Drop a tab into a stack or edge split.
    DropTab {
        /// Dragged tab identity.
        payload: DockDragPayload,
        /// Drop destination.
        target: DockDropTarget,
    },
    /// Close a tab.
    CloseTab(DockTabSelection),
    /// Close an entire stack.
    CloseStack(DockStackSelection),
    /// Change tab pinning.
    PinTab {
        /// Target tab.
        selection: DockTabSelection,
        /// New pinned state.
        pinned: bool,
    },
    /// Change stack pinning.
    PinStack {
        /// Target stack.
        selection: DockStackSelection,
        /// New pinned state.
        pinned: bool,
    },
    /// Resize a split.
    ResizeSplit(DockSplitResize),
}

/// Selected dock-tab metadata emitted by host-managed dock interactions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockTabSelection {
    stack_id: SharedString,
    tab_id: SharedString,
}

impl DockTabSelection {
    /// Creates a new dock-tab selection descriptor.
    #[must_use]
    pub fn new(stack_id: impl Into<SharedString>, tab_id: impl Into<SharedString>) -> Self {
        Self {
            stack_id: stack_id.into(),
            tab_id: tab_id.into(),
        }
    }

    /// Returns the source stack identifier.
    #[must_use]
    pub fn stack_id(&self) -> &SharedString {
        &self.stack_id
    }

    /// Returns the selected tab identifier.
    #[must_use]
    pub fn tab_id(&self) -> &SharedString {
        &self.tab_id
    }
}

/// Selected dock-stack metadata emitted by host-managed dock interactions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockStackSelection {
    stack_id: SharedString,
}

/// Split resize metadata emitted by host-managed dock interactions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSplitResize {
    stack_id: SharedString,
    axis: DockAxis,
    ratio: u16,
}

impl DockSplitResize {
    /// Creates a split resize request.
    #[must_use]
    pub fn new(stack_id: impl Into<SharedString>, axis: DockAxis, ratio: u16) -> Self {
        Self {
            stack_id: stack_id.into(),
            axis,
            ratio: clamp_ratio(ratio),
        }
    }

    /// Returns the first stack identifier inside the split being resized.
    #[must_use]
    pub fn stack_id(&self) -> &SharedString {
        &self.stack_id
    }

    /// Returns the split axis.
    #[must_use]
    pub fn axis(&self) -> DockAxis {
        self.axis
    }

    /// Returns the requested ratio for the first side of the split.
    #[must_use]
    pub fn ratio(&self) -> u16 {
        self.ratio
    }
}

impl DockStackSelection {
    /// Creates a new dock-stack selection descriptor.
    #[must_use]
    pub fn new(stack_id: impl Into<SharedString>) -> Self {
        Self {
            stack_id: stack_id.into(),
        }
    }

    /// Returns the source stack identifier.
    #[must_use]
    pub fn stack_id(&self) -> &SharedString {
        &self.stack_id
    }
}

/// Immutable tab metadata for a dock leaf.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockTab {
    id: SharedString,
    title: SharedString,
    body: SharedString,
    badge: Option<SharedString>,
    pinned: bool,
}

impl DockTab {
    /// Creates a new dock tab.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            badge: None,
            pinned: false,
        }
    }

    /// Attaches an optional badge label to the tab.
    #[must_use]
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Sets whether the tab is pinned in its stack.
    #[must_use]
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Returns the stable tab identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the tab title.
    #[must_use]
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Returns whether the tab is pinned.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }
}

/// A stacked leaf in a dock layout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockTabs {
    id: SharedString,
    tabs: Vec<DockTab>,
    active_tab: usize,
    #[serde(default)]
    pinned: bool,
}

impl DockTabs {
    /// Creates a new tab stack.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, tabs: Vec<DockTab>) -> Self {
        Self {
            id: id.into(),
            tabs,
            active_tab: 0,
            pinned: false,
        }
    }

    /// Sets the active tab index.
    #[must_use]
    pub fn active_tab(mut self, active_tab: usize) -> Self {
        self.active_tab = active_tab;
        self.normalize();
        self
    }

    /// Sets whether the stack is protected from close and empty collapse.
    #[must_use]
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Returns the active tab when present.
    #[must_use]
    pub fn active(&self) -> Option<&DockTab> {
        self.tabs.get(self.active_tab)
    }

    /// Returns the stable stack identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns the immutable tab slice.
    #[must_use]
    pub fn tabs(&self) -> &[DockTab] {
        &self.tabs
    }

    /// Returns the active tab index.
    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// Returns whether this stack is protected from close and empty collapse.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Selects the next tab, wrapping at the end.
    pub fn select_next(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
        true
    }

    /// Selects the previous tab, wrapping at the beginning.
    pub fn select_previous(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        self.active_tab = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        true
    }

    /// Moves a tab within the stack.
    pub fn move_tab(&mut self, tab_id: &str, target_index: usize) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id.as_ref() == tab_id) else {
            return false;
        };
        let tab = self.tabs.remove(index);
        let target_index = target_index.min(self.tabs.len());
        self.tabs.insert(target_index, tab);
        self.active_tab = target_index;
        self.sort_pinned_prefix();
        self.normalize();
        true
    }

    /// Sets pinned state for a tab in the stack.
    pub fn pin_tab(&mut self, tab_id: &str, pinned: bool) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id.as_ref() == tab_id) else {
            return false;
        };
        tab.pinned = pinned;
        self.sort_pinned_prefix();
        self.normalize();
        true
    }

    fn normalize(&mut self) {
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else {
            self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
        }
    }

    fn sort_pinned_prefix(&mut self) {
        let active_id = self.active().map(|tab| tab.id.clone());
        self.tabs.sort_by_key(|tab| !tab.pinned);
        if let Some(active_id) = active_id
            && let Some(index) = self.tabs.iter().position(|tab| tab.id == active_id)
        {
            self.active_tab = index;
        }
    }
}

/// A recursive dock layout node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DockNode {
    /// Split layout with two child nodes.
    Split {
        /// Split axis.
        axis: DockAxis,
        /// Ratio assigned to the first child.
        ratio: u16,
        /// First child node.
        first: Box<DockNode>,
        /// Second child node.
        second: Box<DockNode>,
    },
    /// Tab stack leaf.
    Tabs(DockTabs),
}

impl DockNode {
    /// Creates a horizontal split.
    #[must_use]
    pub fn horizontal(first: DockNode, second: DockNode, ratio: u16) -> Self {
        Self::Split {
            axis: DockAxis::Horizontal,
            ratio: clamp_ratio(ratio),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Creates a vertical split.
    #[must_use]
    pub fn vertical(first: DockNode, second: DockNode, ratio: u16) -> Self {
        Self::Split {
            axis: DockAxis::Vertical,
            ratio: clamp_ratio(ratio),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Returns the number of leaf tab stacks in the node tree.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
            Self::Tabs(_) => 1,
        }
    }

    /// Returns the number of tabs in the node tree.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        match self {
            Self::Split { first, second, .. } => first.tab_count() + second.tab_count(),
            Self::Tabs(tabs) => tabs.tabs.len(),
        }
    }

    /// Selects the provided tab when found in the layout tree.
    pub fn select_tab(&mut self, stack_id: &str, tab_id: &str) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.select_tab(stack_id, tab_id) || second.select_tab(stack_id, tab_id)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                if let Some(index) = stack.tabs.iter().position(|tab| tab.id.as_ref() == tab_id) {
                    stack.active_tab = index;
                    true
                } else {
                    false
                }
            }
            Self::Tabs(_) => false,
        }
    }

    /// Updates a split ratio when found in the layout tree.
    pub fn resize_split(&mut self, target_stack_id: &str, ratio: u16) -> bool {
        match self {
            Self::Split {
                ratio: current,
                first,
                second,
                ..
            } => {
                if first.contains_stack(target_stack_id) {
                    *current = clamp_ratio(ratio);
                    true
                } else if second.contains_stack(target_stack_id) {
                    *current = clamp_ratio(1000_u16.saturating_sub(ratio));
                    true
                } else {
                    first.resize_split(target_stack_id, ratio)
                        || second.resize_split(target_stack_id, ratio)
                }
            }
            Self::Tabs(_) => false,
        }
    }

    /// Removes a tab from the given stack and returns it.
    pub fn remove_tab(&mut self, stack_id: &str, tab_id: &str) -> Option<DockTab> {
        match self {
            Self::Split { first, second, .. } => first
                .remove_tab(stack_id, tab_id)
                .or_else(|| second.remove_tab(stack_id, tab_id)),
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                let index = stack
                    .tabs
                    .iter()
                    .position(|tab| tab.id.as_ref() == tab_id)?;
                let tab = stack.tabs.remove(index);
                stack.normalize();
                Some(tab)
            }
            Self::Tabs(_) => None,
        }
    }

    /// Removes an entire tab stack and returns it.
    pub fn remove_stack(&mut self, stack_id: &str) -> Option<DockTabs> {
        match self {
            Self::Split { first, second, .. } => first
                .remove_stack(stack_id)
                .or_else(|| second.remove_stack(stack_id)),
            Self::Tabs(stack) if stack.id.as_ref() == stack_id && !stack.pinned => {
                Some(stack.clone())
            }
            Self::Tabs(_) => None,
        }
    }

    /// Inserts a tab into the target stack and selects it.
    pub fn insert_tab(&mut self, stack_id: &str, tab: DockTab) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.insert_tab(stack_id, tab.clone()) || second.insert_tab(stack_id, tab)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                stack.tabs.push(tab);
                stack.active_tab = stack.tabs.len().saturating_sub(1);
                stack.sort_pinned_prefix();
                true
            }
            Self::Tabs(_) => false,
        }
    }

    /// Moves a tab within the target stack.
    pub fn move_tab_within_stack(
        &mut self,
        stack_id: &str,
        tab_id: &str,
        target_index: usize,
    ) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.move_tab_within_stack(stack_id, tab_id, target_index)
                    || second.move_tab_within_stack(stack_id, tab_id, target_index)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                stack.move_tab(tab_id, target_index)
            }
            Self::Tabs(_) => false,
        }
    }

    /// Sets pinned state for a tab.
    pub fn pin_tab(&mut self, stack_id: &str, tab_id: &str, pinned: bool) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.pin_tab(stack_id, tab_id, pinned) || second.pin_tab(stack_id, tab_id, pinned)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => stack.pin_tab(tab_id, pinned),
            Self::Tabs(_) => false,
        }
    }

    /// Sets pinned state for a stack.
    pub fn pin_stack(&mut self, stack_id: &str, pinned: bool) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.pin_stack(stack_id, pinned) || second.pin_stack(stack_id, pinned)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id && stack.pinned != pinned => {
                stack.pinned = pinned;
                true
            }
            Self::Tabs(_) => false,
        }
    }

    /// Selects an adjacent tab in a stack.
    pub fn select_adjacent_tab(&mut self, stack_id: &str, direction: i8) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.select_adjacent_tab(stack_id, direction)
                    || second.select_adjacent_tab(stack_id, direction)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                if direction < 0 {
                    stack.select_previous()
                } else {
                    stack.select_next()
                }
            }
            Self::Tabs(_) => false,
        }
    }

    fn clear_stack(&mut self, stack_id: &str) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.clear_stack(stack_id) || second.clear_stack(stack_id)
            }
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => {
                stack.tabs.clear();
                stack.normalize();
                true
            }
            Self::Tabs(_) => false,
        }
    }

    /// Splits the target stack and inserts a new stack containing the provided tab.
    pub fn split_with_tab(
        &mut self,
        target_stack_id: &str,
        placement: DockPlacement,
        new_stack_id: impl Into<SharedString>,
        tab: DockTab,
    ) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                let new_stack_id = new_stack_id.into();
                first.split_with_tab(
                    target_stack_id,
                    placement,
                    new_stack_id.clone(),
                    tab.clone(),
                ) || second.split_with_tab(target_stack_id, placement, new_stack_id, tab)
            }
            Self::Tabs(stack) if stack.id.as_ref() == target_stack_id => {
                let existing = Self::Tabs(stack.clone());
                let inserted = Self::Tabs(DockTabs::new(new_stack_id, vec![tab]));
                *self = match placement {
                    DockPlacement::Left => Self::horizontal(inserted, existing, 320),
                    DockPlacement::Right => Self::horizontal(existing, inserted, 680),
                    DockPlacement::Top => Self::vertical(inserted, existing, 320),
                    DockPlacement::Bottom => Self::vertical(existing, inserted, 680),
                };
                true
            }
            Self::Tabs(_) => false,
        }
    }

    fn contains_stack(&self, stack_id: &str) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.contains_stack(stack_id) || second.contains_stack(stack_id)
            }
            Self::Tabs(stack) => stack.id.as_ref() == stack_id,
        }
    }

    fn tab_index(&self, stack_id: &str, tab_id: &str) -> Option<(usize, usize)> {
        match self {
            Self::Split { first, second, .. } => first
                .tab_index(stack_id, tab_id)
                .or_else(|| second.tab_index(stack_id, tab_id)),
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => stack
                .tabs
                .iter()
                .position(|tab| tab.id.as_ref() == tab_id)
                .map(|index| (index, stack.tabs.len())),
            Self::Tabs(_) => None,
        }
    }

    fn first_stack_id(&self) -> Option<SharedString> {
        match self {
            Self::Split { first, .. } => first.first_stack_id(),
            Self::Tabs(stack) => Some(stack.id.clone()),
        }
    }

    fn active_selection(&self) -> Option<DockTabSelection> {
        match self {
            Self::Split { first, second, .. } => first
                .active_selection()
                .or_else(|| second.active_selection()),
            Self::Tabs(stack) => stack
                .active()
                .map(|tab| DockTabSelection::new(stack.id.clone(), tab.id.clone())),
        }
    }

    fn active_selection_in(&self, stack_id: &str) -> Option<DockTabSelection> {
        match self {
            Self::Split { first, second, .. } => first
                .active_selection_in(stack_id)
                .or_else(|| second.active_selection_in(stack_id)),
            Self::Tabs(stack) if stack.id.as_ref() == stack_id => stack
                .active()
                .map(|tab| DockTabSelection::new(stack.id.clone(), tab.id.clone())),
            Self::Tabs(_) => None,
        }
    }

    fn collect_stack_ids(&self, ids: &mut Vec<SharedString>) {
        match self {
            Self::Split { first, second, .. } => {
                first.collect_stack_ids(ids);
                second.collect_stack_ids(ids);
            }
            Self::Tabs(stack) => ids.push(stack.id.clone()),
        }
    }

    fn is_empty_tabs(&self) -> bool {
        matches!(self, Self::Tabs(stack) if stack.tabs.is_empty() && !stack.pinned)
    }

    fn normalize(&mut self) {
        if let Self::Split { first, second, .. } = self {
            first.normalize();
            second.normalize();

            if first.is_empty_tabs() && !second.is_empty_tabs() {
                *self = (**second).clone();
            } else if second.is_empty_tabs() && !first.is_empty_tabs() {
                *self = (**first).clone();
            }
        }
    }
}

/// Serializable dock layout state with split panels and stacked tabs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DockLayout {
    root: DockNode,
    #[serde(default)]
    focused_stack_id: Option<SharedString>,
}

impl DockLayout {
    /// Creates a new dock layout.
    #[must_use]
    pub fn new(root: DockNode) -> Self {
        let focused_stack_id = root.first_stack_id();
        Self {
            root,
            focused_stack_id,
        }
    }

    /// Returns the root node.
    #[must_use]
    pub fn root(&self) -> &DockNode {
        &self.root
    }

    /// Returns the number of leaf stacks.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.root.leaf_count()
    }

    /// Returns the total number of tabs.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.root.tab_count()
    }

    /// Returns the stack currently targeted by keyboard commands.
    #[must_use]
    pub fn focused_stack_id(&self) -> Option<&SharedString> {
        self.focused_stack_id.as_ref()
    }

    /// Returns the active tab in the first non-empty stack in layout order.
    #[must_use]
    pub fn active_selection(&self) -> Option<DockTabSelection> {
        self.root.active_selection()
    }

    /// Returns the active tab for a specific stack.
    #[must_use]
    pub fn active_selection_in(&self, stack_id: &str) -> Option<DockTabSelection> {
        self.root.active_selection_in(stack_id)
    }

    /// Selects a tab in the given stack when found.
    pub fn select_tab(&mut self, stack_id: &str, tab_id: &str) -> bool {
        let changed = self.root.select_tab(stack_id, tab_id);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Updates the split ratio associated with the branch that contains the stack.
    pub fn resize_split(&mut self, stack_id: &str, ratio: u16) -> bool {
        let changed = self.root.resize_split(stack_id, ratio);
        self.normalize();
        changed
    }

    /// Returns stack identifiers in layout order.
    #[must_use]
    pub fn stack_ids(&self) -> Vec<SharedString> {
        let mut ids = Vec::new();
        self.root.collect_stack_ids(&mut ids);
        ids
    }

    /// Selects the next or previous tab in a stack, wrapping at the ends.
    pub fn select_adjacent_tab(&mut self, stack_id: &str, direction: i8) -> bool {
        let changed = self.root.select_adjacent_tab(stack_id, direction);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Focuses a stack when it exists.
    pub fn focus_stack(&mut self, stack_id: &str) -> bool {
        if !self.root.contains_stack(stack_id) || self.focused_stack_id.as_deref() == Some(stack_id)
        {
            return false;
        }
        self.focused_stack_id = Some(stack_id.to_owned().into());
        true
    }

    /// Focuses the previous or next stack in layout order, wrapping at edges.
    pub fn focus_adjacent_stack(&mut self, stack_id: &str, direction: i8) -> bool {
        let Some(target) = self.adjacent_stack_id(stack_id, direction) else {
            return false;
        };
        self.focus_stack(&target)
    }

    /// Moves a tab to the previous or next stack in layout order.
    pub fn move_tab_to_adjacent_stack(
        &mut self,
        stack_id: &str,
        tab_id: &str,
        direction: i8,
    ) -> bool {
        let Some(target) = self.adjacent_stack_id(stack_id, direction) else {
            return false;
        };
        self.move_tab_to_stack(stack_id, tab_id, &target)
    }

    /// Moves a tab within a stack and selects it at the new position.
    pub fn move_tab_within_stack(
        &mut self,
        stack_id: &str,
        tab_id: &str,
        target_index: usize,
    ) -> bool {
        let changed = self
            .root
            .move_tab_within_stack(stack_id, tab_id, target_index);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Sets pinned state for a tab and keeps pinned tabs before unpinned tabs.
    pub fn pin_tab(&mut self, stack_id: &str, tab_id: &str, pinned: bool) -> bool {
        let changed = self.root.pin_tab(stack_id, tab_id, pinned);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Pins or unpins a stack.
    ///
    /// Pinned stacks reject [`Self::close_stack`] and remain in the split tree
    /// when their final tab is closed or moved away.
    pub fn pin_stack(&mut self, stack_id: &str, pinned: bool) -> bool {
        let changed = self.root.pin_stack(stack_id, pinned);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Moves an existing tab into another tab stack and selects it there.
    pub fn move_tab_to_stack(
        &mut self,
        from_stack_id: &str,
        tab_id: &str,
        to_stack_id: &str,
    ) -> bool {
        if from_stack_id == to_stack_id {
            return false;
        }
        let Some(tab) = self.root.remove_tab(from_stack_id, tab_id) else {
            return false;
        };

        let changed = if self.root.insert_tab(to_stack_id, tab.clone()) {
            true
        } else {
            let _ = self.root.insert_tab(from_stack_id, tab);
            false
        };
        if changed {
            self.focused_stack_id = Some(to_stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Inserts a tab into an existing stack and selects it.
    pub fn insert_tab(&mut self, stack_id: &str, tab: DockTab) -> bool {
        let changed = self.root.insert_tab(stack_id, tab);
        if changed {
            self.focused_stack_id = Some(stack_id.to_owned().into());
        }
        self.normalize();
        changed
    }

    /// Closes a tab in the given stack and collapses empty stacks automatically.
    pub fn close_tab(&mut self, stack_id: &str, tab_id: &str) -> bool {
        let changed = self.root.remove_tab(stack_id, tab_id).is_some();
        self.normalize();
        changed
    }

    /// Closes an entire tab stack and collapses adjacent splits when possible.
    pub fn close_stack(&mut self, stack_id: &str) -> bool {
        let changed = self.root.remove_stack(stack_id).is_some();
        if changed {
            let _ = self.root.clear_stack(stack_id);
        }
        self.normalize();
        changed
    }

    /// Splits the target stack and creates a new stack containing the moved tab.
    pub fn split_stack_with_moved_tab(
        &mut self,
        from_stack_id: &str,
        tab_id: &str,
        target_stack_id: &str,
        placement: DockPlacement,
        new_stack_id: impl Into<SharedString>,
    ) -> bool {
        if from_stack_id == target_stack_id
            && matches!(self.root.tab_index(from_stack_id, tab_id), Some((_, 1)))
        {
            return false;
        }
        let new_stack_id = new_stack_id.into();
        let Some(tab) = self.root.remove_tab(from_stack_id, tab_id) else {
            return false;
        };

        let changed = if self.root.split_with_tab(
            target_stack_id,
            placement,
            new_stack_id.clone(),
            tab.clone(),
        ) {
            true
        } else {
            let _ = self.root.insert_tab(from_stack_id, tab);
            false
        };
        if changed {
            self.focused_stack_id = Some(new_stack_id);
        }
        self.normalize();
        changed
    }

    /// Splits the target stack and creates a new stack containing a new tab.
    pub fn split_stack_with_tab(
        &mut self,
        target_stack_id: &str,
        placement: DockPlacement,
        new_stack_id: impl Into<SharedString>,
        tab: DockTab,
    ) -> bool {
        let new_stack_id = new_stack_id.into();
        let changed =
            self.root
                .split_with_tab(target_stack_id, placement, new_stack_id.clone(), tab);
        if changed {
            self.focused_stack_id = Some(new_stack_id);
        }
        self.normalize();
        changed
    }

    /// Applies a pointer- or keyboard-originated dock command.
    pub fn apply(&mut self, command: &DockCommand) -> bool {
        match command {
            DockCommand::SelectTab(selection) => {
                self.select_tab(selection.stack_id(), selection.tab_id())
            }
            DockCommand::SelectAdjacentTab {
                stack_id,
                direction,
            } => self.select_adjacent_tab(stack_id, *direction),
            DockCommand::MoveTab {
                stack_id,
                tab_id,
                delta,
            } => {
                let Some((index, len)) = self.root.tab_index(stack_id, tab_id) else {
                    return false;
                };
                let target = index
                    .saturating_add_signed(*delta)
                    .min(len.saturating_sub(1));
                self.move_tab_within_stack(stack_id, tab_id, target)
            }
            DockCommand::FocusAdjacentStack {
                stack_id,
                direction,
            } => self.focus_adjacent_stack(stack_id, *direction),
            DockCommand::MoveTabToAdjacentStack {
                stack_id,
                tab_id,
                direction,
            } => self.move_tab_to_adjacent_stack(stack_id, tab_id, *direction),
            DockCommand::SplitTab {
                selection,
                placement,
                new_stack_id,
            } => self.split_stack_with_moved_tab(
                selection.stack_id(),
                selection.tab_id(),
                selection.stack_id(),
                *placement,
                self.unique_stack_id(new_stack_id),
            ),
            DockCommand::DropTab { payload, target } => match target.zone {
                DockDropZone::Center => {
                    self.move_tab_to_stack(&payload.stack_id, &payload.tab_id, &target.stack_id)
                }
                zone => self.split_stack_with_moved_tab(
                    &payload.stack_id,
                    &payload.tab_id,
                    &target.stack_id,
                    placement_for_drop_zone(zone),
                    self.unique_stack_id(&target.new_stack_id),
                ),
            },
            DockCommand::CloseTab(selection) => {
                self.close_tab(selection.stack_id(), selection.tab_id())
            }
            DockCommand::CloseStack(selection) => self.close_stack(selection.stack_id()),
            DockCommand::PinTab { selection, pinned } => {
                self.pin_tab(selection.stack_id(), selection.tab_id(), *pinned)
            }
            DockCommand::PinStack { selection, pinned } => {
                self.pin_stack(selection.stack_id(), *pinned)
            }
            DockCommand::ResizeSplit(resize) => {
                self.resize_split(resize.stack_id(), resize.ratio())
            }
        }
    }

    fn unique_stack_id(&self, preferred: &str) -> SharedString {
        if !self.root.contains_stack(preferred) {
            return preferred.to_owned().into();
        }
        for suffix in 2.. {
            let candidate = format!("{preferred}-{suffix}");
            if !self.root.contains_stack(&candidate) {
                return candidate.into();
            }
        }
        unreachable!("an unbounded numeric suffix always yields a unique stack identifier")
    }

    fn adjacent_stack_id(&self, stack_id: &str, direction: i8) -> Option<SharedString> {
        let stack_ids = self.stack_ids();
        if stack_ids.len() < 2 {
            return None;
        }
        let index = stack_ids.iter().position(|id| id.as_ref() == stack_id)?;
        let target = if direction < 0 {
            index.checked_sub(1).unwrap_or(stack_ids.len() - 1)
        } else {
            (index + 1) % stack_ids.len()
        };
        stack_ids.get(target).cloned()
    }

    fn normalize(&mut self) {
        self.root.normalize();
        if self
            .focused_stack_id
            .as_deref()
            .is_none_or(|id| !self.root.contains_stack(id))
        {
            self.focused_stack_id = self.root.first_stack_id();
        }
    }

    /// Serializes the layout to JSON for persistence.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Restores a persisted layout from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut layout: Self = serde_json::from_str(json)?;
        layout.normalize();
        Ok(layout)
    }
}

fn placement_for_drop_zone(zone: DockDropZone) -> DockPlacement {
    match zone {
        DockDropZone::Left => DockPlacement::Left,
        DockDropZone::Right => DockPlacement::Right,
        DockDropZone::Top => DockPlacement::Top,
        DockDropZone::Bottom => DockPlacement::Bottom,
        DockDropZone::Center => unreachable!("center drops do not create splits"),
    }
}

/// A production-oriented dock surface with split panels and stacked tabs.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Dock, DockLayout, DockNode, DockTab, DockTabs};
///
/// let layout = DockLayout::new(DockNode::horizontal(
///     DockNode::Tabs(DockTabs::new(
///         "left",
///         vec![DockTab::new("files", "Files", "Project files")],
///     )),
///     DockNode::Tabs(DockTabs::new(
///         "main",
///         vec![DockTab::new("editor", "Editor", "Main editor surface")],
///     )),
///     280,
/// ));
///
/// let dock = Dock::new("workspace-dock", layout);
/// ```
#[derive(gpui::IntoElement)]
pub struct Dock {
    id: SharedString,
    title: Option<SharedString>,
    layout: DockLayout,
    on_command: Option<DockCommandHandler>,
    focus_handle: Option<FocusHandle>,
    keyboard_stack_id: Option<SharedString>,
    tab_body_renderer: Option<DockTabBodyRenderer>,
    tab_scroll_handles: HashMap<SharedString, ScrollHandle>,
}

impl Dock {
    /// Creates a new dock surface.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, layout: DockLayout) -> Self {
        Self {
            id: id.into(),
            title: None,
            layout,
            on_command: None,
            focus_handle: None,
            keyboard_stack_id: None,
            tab_body_renderer: None,
            tab_scroll_handles: HashMap::new(),
        }
    }

    /// Sets an optional title above the dock.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Handles every dock interaction through a single command stream.
    ///
    /// The host normally applies each command with [`DockLayout::apply`] and
    /// then requests a render. Installing this handler also enables tab close,
    /// stack close, split resize, and native tab drag-and-drop affordances.
    #[must_use]
    pub fn on_command(
        mut self,
        handler: impl Fn(&DockCommand, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_command = Some(Rc::new(handler));
        self
    }

    /// Makes the dock keyboard-focusable.
    ///
    /// `Control`/`Command` + `Tab` selects the next tab (add `Shift` for the
    /// previous tab). `Control`/`Command` + `Shift` + arrow moves the active
    /// tab within its stack. The host owns the focus handle so focus survives
    /// controlled re-renders.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Selects the stack targeted by dock keyboard commands.
    ///
    /// If omitted, the first non-empty stack in layout order is used.
    #[must_use]
    pub fn keyboard_stack(mut self, stack_id: impl Into<SharedString>) -> Self {
        self.keyboard_stack_id = Some(stack_id.into());
        self
    }

    /// Tracks horizontal tab overflow for a stack with a persistent handle.
    ///
    /// The host should retain the handle across controlled re-renders when it
    /// wants to preserve the user's tab-strip scroll position.
    #[must_use]
    pub fn track_tab_scroll(
        mut self,
        stack_id: impl Into<SharedString>,
        handle: ScrollHandle,
    ) -> Self {
        self.tab_scroll_handles.insert(stack_id.into(), handle);
        self
    }

    /// Sets a custom renderer for active tab bodies.
    ///
    /// Use this when dock tabs represent live surfaces such as terminals,
    /// editors, inspectors, or charts instead of static text.
    #[must_use]
    pub fn render_tab_body(
        mut self,
        renderer: impl Fn(&DockTabSelection, &DockTab) -> AnyElement + 'static,
    ) -> Self {
        self.tab_body_renderer = Some(Rc::new(renderer));
        self
    }
}

impl RenderOnce for Dock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx).clone();
        let root_selector = format!("guic-dock-root-{}", self.id);
        let active_selection = self
            .keyboard_stack_id
            .as_deref()
            .and_then(|stack_id| self.layout.active_selection_in(stack_id))
            .or_else(|| {
                self.layout
                    .focused_stack_id()
                    .and_then(|stack_id| self.layout.active_selection_in(stack_id))
            })
            .or_else(|| self.layout.active_selection());
        let mut root = div()
            .id(self.id)
            .debug_selector({
                let root_selector = root_selector.clone();
                move || root_selector.clone()
            })
            .w_full()
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .p_3()
            .flex()
            .flex_col()
            .gap_3();

        if let Some(handle) = &self.focus_handle {
            root = root.key_context("GuicDock").track_focus(handle);
        }

        if let (Some(handler), Some(selection)) = (self.on_command.clone(), active_selection) {
            root = root.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let modifiers = &event.keystroke.modifiers;
                let command_modifier = modifiers.control || modifiers.platform;
                let command = match event.keystroke.key.as_str() {
                    "tab" if command_modifier => Some(DockCommand::SelectAdjacentTab {
                        stack_id: selection.stack_id().clone(),
                        direction: if modifiers.shift { -1 } else { 1 },
                    }),
                    "w" if command_modifier && modifiers.shift => Some(DockCommand::CloseStack(
                        DockStackSelection::new(selection.stack_id().clone()),
                    )),
                    "w" if command_modifier => Some(DockCommand::CloseTab(selection.clone())),
                    "enter" if command_modifier && modifiers.shift && modifiers.alt => {
                        Some(DockCommand::SplitTab {
                            selection: selection.clone(),
                            placement: DockPlacement::Bottom,
                            new_stack_id: format!(
                                "{}-{}-bottom",
                                selection.stack_id(),
                                selection.tab_id()
                            )
                            .into(),
                        })
                    }
                    "enter" if command_modifier && modifiers.shift => Some(DockCommand::SplitTab {
                        selection: selection.clone(),
                        placement: DockPlacement::Right,
                        new_stack_id: format!(
                            "{}-{}-right",
                            selection.stack_id(),
                            selection.tab_id()
                        )
                        .into(),
                    }),
                    "left" | "up" if command_modifier && modifiers.alt && modifiers.shift => {
                        Some(DockCommand::MoveTabToAdjacentStack {
                            stack_id: selection.stack_id().clone(),
                            tab_id: selection.tab_id().clone(),
                            direction: -1,
                        })
                    }
                    "right" | "down" if command_modifier && modifiers.alt && modifiers.shift => {
                        Some(DockCommand::MoveTabToAdjacentStack {
                            stack_id: selection.stack_id().clone(),
                            tab_id: selection.tab_id().clone(),
                            direction: 1,
                        })
                    }
                    "left" | "up" if command_modifier && modifiers.alt => {
                        Some(DockCommand::FocusAdjacentStack {
                            stack_id: selection.stack_id().clone(),
                            direction: -1,
                        })
                    }
                    "right" | "down" if command_modifier && modifiers.alt => {
                        Some(DockCommand::FocusAdjacentStack {
                            stack_id: selection.stack_id().clone(),
                            direction: 1,
                        })
                    }
                    "left" | "up" if command_modifier && modifiers.shift => {
                        Some(DockCommand::MoveTab {
                            stack_id: selection.stack_id().clone(),
                            tab_id: selection.tab_id().clone(),
                            delta: -1,
                        })
                    }
                    "right" | "down" if command_modifier && modifiers.shift => {
                        Some(DockCommand::MoveTab {
                            stack_id: selection.stack_id().clone(),
                            tab_id: selection.tab_id().clone(),
                            delta: 1,
                        })
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    handler(&command, window, cx);
                }
            });
        }

        if let Some(title) = self.title {
            root = root.child(Label::new(title).muted(true));
        }

        root.child(render_dock_node(
            self.layout.root,
            &theme,
            DockRenderHandlers {
                on_command: self.on_command.clone(),
                tab_body_renderer: self.tab_body_renderer.clone(),
                tab_scroll_handles: Rc::new(self.tab_scroll_handles),
                focus_handle: self.focus_handle.clone(),
                focused_stack_id: self.layout.focused_stack_id.clone(),
            },
        ))
    }
}

fn render_dock_node(node: DockNode, theme: &Theme, handlers: DockRenderHandlers) -> AnyElement {
    match node {
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let first_stack_id = first.first_stack_id();
            let primary_share = ratio as f32 / 1000.0;
            let secondary_share = 1.0 - primary_share;
            let split_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
            let split_bounds_sink = split_bounds.clone();
            let bounds_canvas = canvas(
                move |bounds, _window, _cx| {
                    split_bounds_sink.set(Some(bounds));
                },
                |_bounds, _state, _window, _cx| {},
            )
            .absolute()
            .inset_0();
            let mut root = div()
                .relative()
                .w_full()
                .h_full()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .flex()
                .gap_3()
                .child(bounds_canvas);

            root = match axis {
                DockAxis::Horizontal => root.flex_row(),
                DockAxis::Vertical => root.flex_col(),
            };

            root.child(render_weighted_node(
                *first,
                primary_share,
                axis,
                theme,
                handlers.clone(),
            ))
            .child(render_split_handle(
                axis,
                ratio,
                first_stack_id,
                split_bounds,
                &handlers,
            ))
            .child(render_weighted_node(
                *second,
                secondary_share,
                axis,
                theme,
                handlers,
            ))
            .into_any_element()
        }
        DockNode::Tabs(tabs) => render_tabs_leaf(tabs, theme, handlers),
    }
}

fn render_weighted_node(
    node: DockNode,
    share: f32,
    axis: DockAxis,
    theme: &Theme,
    handlers: DockRenderHandlers,
) -> AnyElement {
    let weighted = match axis {
        DockAxis::Horizontal => div()
            .flex_basis(relative(share))
            .flex_shrink_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(render_dock_node(node, theme, handlers)),
        DockAxis::Vertical => div()
            .flex_basis(relative(share))
            .flex_shrink_1()
            .w_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(render_dock_node(node, theme, handlers)),
    };

    weighted.into_any_element()
}

fn render_split_handle(
    axis: DockAxis,
    ratio: u16,
    first_stack_id: Option<SharedString>,
    split_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    handlers: &DockRenderHandlers,
) -> AnyElement {
    let Some(handler) = handlers.on_command.clone() else {
        return match axis {
            DockAxis::Horizontal => Separator::vertical().into_any_element(),
            DockAxis::Vertical => Separator::horizontal().into_any_element(),
        };
    };
    let Some(stack_id) = first_stack_id else {
        return match axis {
            DockAxis::Horizontal => Separator::vertical().into_any_element(),
            DockAxis::Vertical => Separator::horizontal().into_any_element(),
        };
    };

    let selector = format!("guic-dock-split-resize-{stack_id}");
    let mut handle = div()
        .id(selector.clone())
        .debug_selector(move || selector.clone())
        .rounded(px(2.0))
        .bg(gpui::transparent_black())
        .hover(|style: gpui::StyleRefinement| style.bg(gpui::black().opacity(0.08)))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
        .on_click({
            let stack_id = stack_id.clone();
            let handler = handler.clone();
            move |_, window, cx| {
                handler(
                    &DockCommand::ResizeSplit(DockSplitResize::new(
                        stack_id.clone(),
                        axis,
                        ratio.saturating_add(50),
                    )),
                    window,
                    cx,
                );
            }
        })
        .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
            if !event.dragging() {
                return;
            }
            let Some(bounds) = split_bounds.get() else {
                return;
            };
            let raw_ratio = match axis {
                DockAxis::Horizontal => {
                    let width = f32::from(bounds.size.width).max(1.0);
                    ((f32::from(event.position.x - bounds.origin.x) / width) * 1000.0) as u16
                }
                DockAxis::Vertical => {
                    let height = f32::from(bounds.size.height).max(1.0);
                    ((f32::from(event.position.y - bounds.origin.y) / height) * 1000.0) as u16
                }
            };
            handler(
                &DockCommand::ResizeSplit(DockSplitResize::new(stack_id.clone(), axis, raw_ratio)),
                window,
                cx,
            );
        });

    handle = match axis {
        DockAxis::Horizontal => handle.w(px(8.0)).h_full().cursor_col_resize(),
        DockAxis::Vertical => handle.h(px(8.0)).w_full().cursor_row_resize(),
    };

    handle
        .child(match axis {
            DockAxis::Horizontal => Separator::vertical().into_any_element(),
            DockAxis::Vertical => Separator::horizontal().into_any_element(),
        })
        .into_any_element()
}

fn render_tabs_leaf(tabs: DockTabs, theme: &Theme, handlers: DockRenderHandlers) -> AnyElement {
    let active = tabs.active().cloned();
    let mut tab_strip = div()
        .id(format!("guic-dock-tab-strip-{}", tabs.id))
        .debug_selector({
            let stack_id = tabs.id.clone();
            move || format!("guic-dock-tab-strip-{stack_id}")
        })
        .flex_1()
        .min_w_0()
        .flex()
        .gap_2()
        .items_center()
        .overflow_x_scroll();

    if let Some(scroll_handle) = handlers.tab_scroll_handles.get(&tabs.id) {
        scroll_handle.scroll_to_item(tabs.active_tab);
        tab_strip = tab_strip.track_scroll(scroll_handle);
    }

    for (index, tab) in tabs.tabs.iter().enumerate() {
        let selected = index == tabs.active_tab;
        let selection = DockTabSelection::new(tabs.id.clone(), tab.id.clone());

        // The selectable chip and the close control are siblings so a close
        // click never bubbles into tab selection.
        let mut chip = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(theme.radius.md))
            .bg(if selected {
                theme.primary().opacity(0.12)
            } else {
                theme.secondary().opacity(0.08)
            })
            .pl_3()
            .pr_2()
            .py_2();

        let mut label = div()
            .id(format!("guic-dock-tab-{}-{}", tabs.id, tab.id))
            .debug_selector({
                let stack_id = tabs.id.clone();
                let tab_id = tab.id.clone();
                move || format!("guic-dock-tab-{stack_id}-{tab_id}")
            })
            .flex()
            .items_center()
            .gap_2()
            .child(Label::new(tab.title.clone()).muted(!selected));

        if let Some(badge) = &tab.badge {
            label = label.child(Badge::new(badge.clone()).variant(BadgeVariant::Primary));
        }

        if let Some(handler) = handlers.on_command.clone() {
            let drag_payload = DockDragPayload {
                stack_id: tabs.id.clone(),
                tab_id: tab.id.clone(),
            };
            let focus_handle = handlers.focus_handle.clone();
            label = label
                .cursor_pointer()
                .on_drag(drag_payload, |payload, _, _, cx| {
                    cx.new(|_| payload.clone())
                })
                .on_click(move |_, window, cx| {
                    if let Some(handle) = &focus_handle {
                        window.focus(handle, cx);
                    }
                    handler(&DockCommand::SelectTab(selection.clone()), window, cx);
                });
        }

        chip = chip.child(label);

        if let Some(handler) = handlers.on_command.clone() {
            let pin_selection = DockTabSelection::new(tabs.id.clone(), tab.id.clone());
            let pinned = tab.pinned;
            chip = chip.child(
                div()
                    .id(format!("guic-dock-tab-pin-{}-{}", tabs.id, tab.id))
                    .debug_selector({
                        let stack_id = tabs.id.clone();
                        let tab_id = tab.id.clone();
                        move || format!("guic-dock-tab-pin-{stack_id}-{tab_id}")
                    })
                    .px_1()
                    .rounded(px(theme.radius.sm))
                    .text_color(if pinned {
                        theme.primary()
                    } else {
                        theme.muted_foreground()
                    })
                    .hover(|style: gpui::StyleRefinement| style.bg(theme.secondary().opacity(0.3)))
                    .cursor_pointer()
                    .child(if pinned { "Unpin" } else { "Pin" })
                    .on_click(move |_, window, cx| {
                        handler(
                            &DockCommand::PinTab {
                                selection: pin_selection.clone(),
                                pinned: !pinned,
                            },
                            window,
                            cx,
                        );
                    }),
            );
        }

        if let Some(handler) = handlers.on_command.clone() {
            let close_selection = DockTabSelection::new(tabs.id.clone(), tab.id.clone());
            chip = chip.child(
                div()
                    .id(format!("guic-dock-tab-close-{}-{}", tabs.id, tab.id))
                    .debug_selector({
                        let stack_id = tabs.id.clone();
                        let tab_id = tab.id.clone();
                        move || format!("guic-dock-tab-close-{stack_id}-{tab_id}")
                    })
                    .px_1()
                    .rounded(px(theme.radius.sm))
                    .text_color(theme.muted_foreground())
                    .hover(|style: gpui::StyleRefinement| style.bg(theme.secondary().opacity(0.3)))
                    .cursor_pointer()
                    .child("x")
                    .on_click(move |_, window, cx| {
                        handler(&DockCommand::CloseTab(close_selection.clone()), window, cx);
                    }),
            );
        }

        tab_strip = tab_strip.child(chip);
    }

    let mut header = div()
        .w_full()
        .min_w_0()
        .flex()
        .gap_2()
        .items_center()
        .pb_2()
        .border_b_1()
        .border_color(theme.border())
        .child(tab_strip);

    if let Some(handler) = handlers.on_command.clone() {
        let stack_selection = DockStackSelection::new(tabs.id.clone());
        header = header.child(
            div()
                .flex_shrink_0()
                .id(format!("guic-dock-stack-pin-{}", tabs.id))
                .debug_selector({
                    let stack_id = tabs.id.clone();
                    move || format!("guic-dock-stack-pin-{stack_id}")
                })
                .px_2()
                .py_1()
                .rounded(px(theme.radius.sm))
                .text_color(if tabs.pinned {
                    theme.primary()
                } else {
                    theme.muted_foreground()
                })
                .hover(|style: gpui::StyleRefinement| style.bg(theme.secondary().opacity(0.3)))
                .cursor_pointer()
                .child(if tabs.pinned { "Unpin" } else { "Pin" })
                .on_click({
                    let handler = handler.clone();
                    let stack_selection = stack_selection.clone();
                    let pinned = tabs.pinned;
                    move |_, window, cx| {
                        handler(
                            &DockCommand::PinStack {
                                selection: stack_selection.clone(),
                                pinned: !pinned,
                            },
                            window,
                            cx,
                        );
                    }
                }),
        );

        if !tabs.pinned {
            header = header.child(
                div()
                    .flex_shrink_0()
                    .id(format!("guic-dock-stack-close-{}", tabs.id))
                    .debug_selector({
                        let stack_id = tabs.id.clone();
                        move || format!("guic-dock-stack-close-{stack_id}")
                    })
                    .px_2()
                    .py_1()
                    .rounded(px(theme.radius.sm))
                    .text_color(theme.muted_foreground())
                    .hover(|style: gpui::StyleRefinement| style.bg(theme.secondary().opacity(0.3)))
                    .cursor_pointer()
                    .child("x")
                    .on_click(move |_, window, cx| {
                        handler(
                            &DockCommand::CloseStack(stack_selection.clone()),
                            window,
                            cx,
                        );
                    }),
            );
        }
    }

    let body = active.map_or_else(
        || {
            div()
                .p_4()
                .child(Label::new("No tabs configured").muted(true))
                .into_any_element()
        },
        |tab| {
            let selection = DockTabSelection::new(tabs.id.clone(), tab.id.clone());
            if let Some(renderer) = &handlers.tab_body_renderer {
                renderer(&selection, &tab)
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .child(Label::new(tab.title))
                    .child(Label::new(tab.body).muted(true))
                    .into_any_element()
            }
        },
    );

    let group = SharedString::from(format!("guic-dock-drop-{}", tabs.id));
    let focused = handlers.focused_stack_id.as_ref() == Some(&tabs.id);
    let mut leaf = div()
        .debug_selector({
            let stack_id = tabs.id.clone();
            move || format!("guic-dock-leaf-{stack_id}")
        })
        .group(group.clone())
        .relative()
        .w_full()
        .h_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(if focused {
            theme.primary()
        } else {
            theme.border()
        })
        .bg(theme.background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(body),
        );

    if let Some(handler) = handlers.on_command {
        for zone in [
            DockDropZone::Center,
            DockDropZone::Left,
            DockDropZone::Right,
            DockDropZone::Top,
            DockDropZone::Bottom,
        ] {
            leaf = leaf.child(render_drop_zone(
                tabs.id.clone(),
                zone,
                group.clone(),
                handler.clone(),
                theme,
            ));
        }
    }

    leaf.into_any_element()
}

fn render_drop_zone(
    stack_id: SharedString,
    zone: DockDropZone,
    group: SharedString,
    handler: DockCommandHandler,
    theme: &Theme,
) -> AnyElement {
    let zone_name = match zone {
        DockDropZone::Center => "center",
        DockDropZone::Left => "left",
        DockDropZone::Right => "right",
        DockDropZone::Top => "top",
        DockDropZone::Bottom => "bottom",
    };
    let mut target = div()
        .id(format!("guic-dock-drop-{stack_id}-{zone_name}"))
        .invisible()
        .absolute()
        .rounded(px(theme.radius.md))
        .border_1()
        .border_color(theme.primary())
        .bg(theme.primary().opacity(0.18))
        .group_drag_over::<DockDragPayload>(group, |style| style.visible())
        .on_drop(move |payload: &DockDragPayload, window, cx| {
            let new_stack_id = format!("{}-{}-{}", stack_id, payload.tab_id, zone_name);
            handler(
                &DockCommand::DropTab {
                    payload: payload.clone(),
                    target: DockDropTarget {
                        stack_id: stack_id.clone(),
                        zone,
                        new_stack_id: new_stack_id.into(),
                    },
                },
                window,
                cx,
            );
        });

    target = match zone {
        DockDropZone::Center => target
            .left(relative(0.25))
            .right(relative(0.25))
            .top(relative(0.25))
            .bottom(relative(0.25)),
        DockDropZone::Left => target.left_0().top_0().bottom_0().w(relative(0.25)),
        DockDropZone::Right => target.right_0().top_0().bottom_0().w(relative(0.25)),
        DockDropZone::Top => target
            .top_0()
            .left(relative(0.25))
            .right(relative(0.25))
            .h(relative(0.25)),
        DockDropZone::Bottom => target
            .bottom_0()
            .left(relative(0.25))
            .right(relative(0.25))
            .h(relative(0.25)),
    };
    target.into_any_element()
}

fn clamp_ratio(ratio: u16) -> u16 {
    ratio.clamp(150, 850)
}

#[cfg(test)]
mod tests {
    use super::{
        Dock, DockAxis, DockCommand, DockDragPayload, DockDropTarget, DockDropZone, DockLayout,
        DockNode, DockPlacement, DockTab, DockTabSelection, DockTabs,
    };
    use gpui::{
        AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement, Keystroke,
        Modifiers, ParentElement as _, Render, ScrollHandle, Styled as _, TestAppContext,
        VisualContext as _, Window, div, px,
    };

    fn sample_layout() -> DockLayout {
        DockLayout::new(DockNode::horizontal(
            DockNode::Tabs(DockTabs::new(
                "sidebar",
                vec![
                    DockTab::new("files", "Files", "Project files").badge("3"),
                    DockTab::new("search", "Search", "Workspace search"),
                ],
            )),
            DockNode::vertical(
                DockNode::Tabs(DockTabs::new(
                    "editor",
                    vec![DockTab::new("main", "main.rs", "fn main() {}")],
                )),
                DockNode::Tabs(DockTabs::new(
                    "console",
                    vec![DockTab::new("logs", "Logs", "Build output")],
                )),
                700,
            ),
            280,
        ))
    }

    struct DockHarness {
        layout: DockLayout,
        focus_handle: FocusHandle,
    }

    impl DockHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                layout: sample_layout(),
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl Render for DockHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                Dock::new("dock-harness", self.layout.clone())
                    .on_command(cx.listener(|this, command: &super::DockCommand, _, cx| {
                        this.layout.apply(command);
                        cx.notify();
                    }))
                    .focusable(self.focus_handle.clone()),
            )
        }
    }

    struct CustomDockBodyHarness;

    struct OverflowDockHarness {
        scroll_handle: ScrollHandle,
    }

    impl Render for OverflowDockHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let tabs = (0..24)
                .map(|index| {
                    DockTab::new(format!("tab-{index}"), format!("Long tab {index}"), "body")
                })
                .collect();
            div().w(px(360.0)).child(
                Dock::new(
                    "overflow-dock",
                    DockLayout::new(DockNode::Tabs(DockTabs::new("overflow", tabs))),
                )
                .track_tab_scroll("overflow", self.scroll_handle.clone()),
            )
        }
    }

    impl Render for CustomDockBodyHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                Dock::new("dock-custom-body", sample_layout()).render_tab_body(|selection, tab| {
                    div()
                        .debug_selector({
                            let stack_id = selection.stack_id().clone();
                            let tab_id = selection.tab_id().clone();
                            move || format!("guic-dock-custom-body-{stack_id}-{tab_id}")
                        })
                        .child(format!("custom body: {}", tab.title()))
                        .into_any_element()
                }),
            )
        }
    }

    #[test]
    fn dock_layout_counts_leaves_and_tabs() {
        let layout = sample_layout();
        assert_eq!(layout.leaf_count(), 3);
        assert_eq!(layout.tab_count(), 4);
    }

    #[test]
    fn dock_commands_unify_keyboard_movement_and_pointer_drops() {
        let mut layout = sample_layout();
        assert!(layout.apply(&DockCommand::MoveTab {
            stack_id: "sidebar".into(),
            tab_id: "files".into(),
            delta: 1,
        }));
        assert!(layout.apply(&DockCommand::SelectTab(DockTabSelection::new(
            "sidebar", "files"
        ))));
        assert!(layout.apply(&DockCommand::DropTab {
            payload: DockDragPayload {
                stack_id: "sidebar".into(),
                tab_id: "files".into(),
            },
            target: DockDropTarget {
                stack_id: "editor".into(),
                zone: DockDropZone::Center,
                new_stack_id: "unused".into(),
            },
        }));
        assert_eq!(layout.tab_count(), 4);

        assert!(layout.apply(&DockCommand::DropTab {
            payload: DockDragPayload {
                stack_id: "editor".into(),
                tab_id: "files".into(),
            },
            target: DockDropTarget {
                stack_id: "console".into(),
                zone: DockDropZone::Left,
                new_stack_id: "files-pane".into(),
            },
        }));
        assert_eq!(layout.leaf_count(), 4);
        assert!(layout.stack_ids().iter().any(|id| id == "files-pane"));

        assert!(layout.move_tab_to_stack("files-pane", "files", "editor"));
        assert!(layout.apply(&DockCommand::DropTab {
            payload: DockDragPayload {
                stack_id: "editor".into(),
                tab_id: "files".into(),
            },
            target: DockDropTarget {
                stack_id: "console".into(),
                zone: DockDropZone::Right,
                new_stack_id: "editor".into(),
            },
        }));
        assert!(layout.stack_ids().iter().any(|id| id == "editor-2"));
    }

    #[test]
    fn dock_self_drops_do_not_reorder_or_rename_single_tab_stacks() {
        let mut layout = sample_layout();
        let original = layout.clone();
        assert!(!layout.apply(&DockCommand::DropTab {
            payload: DockDragPayload {
                stack_id: "sidebar".into(),
                tab_id: "files".into(),
            },
            target: DockDropTarget {
                stack_id: "sidebar".into(),
                zone: DockDropZone::Center,
                new_stack_id: "unused".into(),
            },
        }));
        assert_eq!(layout, original);

        assert!(!layout.apply(&DockCommand::DropTab {
            payload: DockDragPayload {
                stack_id: "editor".into(),
                tab_id: "main".into(),
            },
            target: DockDropTarget {
                stack_id: "editor".into(),
                zone: DockDropZone::Right,
                new_stack_id: "renamed-editor".into(),
            },
        }));
        assert_eq!(layout, original);
    }

    #[test]
    fn dock_layout_selects_tabs_and_resizes_splits() {
        let mut layout = sample_layout();
        assert!(layout.select_tab("sidebar", "search"));
        assert!(layout.resize_split("console", 620));
        assert!(!layout.select_tab("missing", "search"));
    }

    #[test]
    fn dock_layout_exposes_stack_ids_in_order() {
        let layout = sample_layout();
        assert_eq!(layout.stack_ids(), vec!["sidebar", "editor", "console"]);
    }

    #[test]
    fn dock_layout_selects_adjacent_tabs() {
        let mut layout = sample_layout();

        assert!(layout.select_adjacent_tab("sidebar", 1));
        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"active_tab\": 1"));

        assert!(layout.select_adjacent_tab("sidebar", -1));
        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"active_tab\": 0"));
    }

    #[test]
    fn dock_layout_focuses_and_moves_tabs_across_stacks() {
        let mut layout = sample_layout();
        assert_eq!(
            layout.focused_stack_id().map(AsRef::as_ref),
            Some("sidebar")
        );

        assert!(layout.apply(&DockCommand::FocusAdjacentStack {
            stack_id: "sidebar".into(),
            direction: 1,
        }));
        assert_eq!(layout.focused_stack_id().map(AsRef::as_ref), Some("editor"));

        assert!(layout.apply(&DockCommand::MoveTabToAdjacentStack {
            stack_id: "sidebar".into(),
            tab_id: "search".into(),
            direction: -1,
        }));
        assert_eq!(
            layout.focused_stack_id().map(AsRef::as_ref),
            Some("console")
        );
        assert!(layout.root.tab_index("console", "search").is_some());
    }

    #[test]
    fn dock_layout_splits_active_tab_through_commands() {
        let mut layout = sample_layout();
        assert!(layout.apply(&DockCommand::SplitTab {
            selection: DockTabSelection::new("sidebar", "search"),
            placement: DockPlacement::Right,
            new_stack_id: "search-split".into(),
        }));
        assert_eq!(layout.leaf_count(), 4);
        assert_eq!(
            layout.focused_stack_id().map(AsRef::as_ref),
            Some("search-split")
        );
    }

    #[test]
    fn dock_layout_moves_tab_within_stack() {
        let mut layout = sample_layout();

        assert!(layout.move_tab_within_stack("sidebar", "search", 0));
        let json = layout.to_json().expect("layout should serialize");
        let search = json.find("\"search\"").expect("search should remain");
        let files = json.find("\"files\"").expect("files should remain");
        assert!(search < files);
    }

    #[test]
    fn dock_layout_pins_tabs_before_unpinned_tabs() {
        let mut layout = sample_layout();

        assert!(layout.pin_tab("sidebar", "search", true));
        let json = layout.to_json().expect("layout should serialize");
        let search = json.find("\"search\"").expect("search should remain");
        let files = json.find("\"files\"").expect("files should remain");
        assert!(search < files);
        assert!(json.contains("\"pinned\": true"));
    }

    #[test]
    fn dock_layout_pinned_stacks_reject_close_and_empty_collapse() {
        let mut layout = sample_layout();
        assert!(layout.apply(&DockCommand::PinStack {
            selection: super::DockStackSelection::new("editor"),
            pinned: true,
        }));
        assert!(!layout.close_stack("editor"));
        assert!(layout.move_tab_to_stack("editor", "main", "console"));
        assert_eq!(layout.leaf_count(), 3);
        assert!(layout.active_selection_in("editor").is_none());

        assert!(layout.pin_stack("editor", false));
        assert_eq!(layout.leaf_count(), 2);
    }

    #[test]
    fn dock_layout_round_trips_json() {
        let layout = sample_layout();
        let json = layout.to_json().expect("layout should serialize");
        let restored = DockLayout::from_json(&json).expect("layout should deserialize");
        assert_eq!(layout, restored);
    }

    #[test]
    fn dock_layout_restores_legacy_json_without_focused_stack() {
        let layout = sample_layout();
        let mut value = serde_json::to_value(layout).expect("layout should serialize");
        value
            .as_object_mut()
            .expect("layout JSON should be an object")
            .remove("focused_stack_id");
        let json = serde_json::to_string(&value).expect("layout JSON should encode");

        let restored = DockLayout::from_json(&json).expect("legacy layout should restore");
        assert_eq!(
            restored.focused_stack_id().map(AsRef::as_ref),
            Some("sidebar")
        );
    }

    #[test]
    fn dock_layout_moves_tabs_between_stacks() {
        let mut layout = sample_layout();
        assert!(layout.move_tab_to_stack("sidebar", "search", "editor"));

        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"search\""));
        assert!(json.contains("\"editor\""));
    }

    #[test]
    fn dock_layout_inserts_tab_into_stack() {
        let mut layout = sample_layout();
        assert!(layout.insert_tab("editor", DockTab::new("terminal", "Terminal", "zsh")));
        assert_eq!(layout.tab_count(), 5);
        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"terminal\""));
        assert!(json.contains("\"active_tab\": 1"));
    }

    #[test]
    fn dock_layout_splits_target_stack_with_moved_tab() {
        let mut layout = sample_layout();
        assert!(layout.split_stack_with_moved_tab(
            "sidebar",
            "search",
            "editor",
            DockPlacement::Bottom,
            "search-panel",
        ));

        assert_eq!(layout.leaf_count(), 4);
        assert_eq!(layout.tab_count(), 4);
    }

    #[test]
    fn dock_layout_splits_target_stack_with_new_tab() {
        let mut layout = sample_layout();
        assert!(layout.split_stack_with_tab(
            "editor",
            DockPlacement::Right,
            "terminal-right",
            DockTab::new("terminal-2", "Terminal 2", "zsh"),
        ));

        assert_eq!(layout.leaf_count(), 4);
        assert_eq!(layout.tab_count(), 5);
        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"terminal-right\""));
        assert!(json.contains("\"terminal-2\""));
    }

    #[test]
    fn dock_layout_collapses_empty_stack_after_move() {
        let mut layout = DockLayout::new(DockNode::horizontal(
            DockNode::Tabs(DockTabs::new(
                "source",
                vec![DockTab::new("files", "Files", "Project files")],
            )),
            DockNode::Tabs(DockTabs::new(
                "target",
                vec![DockTab::new("editor", "Editor", "Main editor")],
            )),
            300,
        ));

        assert!(layout.move_tab_to_stack("source", "files", "target"));
        assert_eq!(layout.leaf_count(), 1);
        assert_eq!(layout.tab_count(), 2);
    }

    #[test]
    fn dock_layout_closes_tab_and_keeps_stack_when_tabs_remain() {
        let mut layout = sample_layout();

        assert!(layout.close_tab("sidebar", "files"));
        assert_eq!(layout.leaf_count(), 3);
        assert_eq!(layout.tab_count(), 3);

        let json = layout.to_json().expect("layout should serialize");
        assert!(json.contains("\"id\": \"search\""));
        assert!(!json.contains("\"id\": \"files\""));
    }

    #[test]
    fn dock_layout_closes_last_tab_and_collapses_stack() {
        let mut layout = DockLayout::new(DockNode::horizontal(
            DockNode::Tabs(DockTabs::new(
                "source",
                vec![DockTab::new("files", "Files", "Project files")],
            )),
            DockNode::Tabs(DockTabs::new(
                "target",
                vec![DockTab::new("editor", "Editor", "Main editor")],
            )),
            300,
        ));

        assert!(layout.close_tab("source", "files"));
        assert_eq!(layout.leaf_count(), 1);
        assert_eq!(layout.tab_count(), 1);
    }

    #[test]
    fn dock_layout_closes_entire_stack() {
        let mut layout = sample_layout();

        assert!(layout.close_stack("console"));
        assert_eq!(layout.leaf_count(), 2);
        assert_eq!(layout.tab_count(), 3);

        let json = layout.to_json().expect("layout should serialize");
        assert!(!json.contains("\"console\""));
        assert!(!json.contains("\"logs\""));
    }

    #[test]
    fn dock_ratio_clamps_to_safe_range() {
        let low = DockNode::horizontal(
            DockNode::Tabs(DockTabs::new("a", vec![DockTab::new("a", "A", "A")])),
            DockNode::Tabs(DockTabs::new("b", vec![DockTab::new("b", "B", "B")])),
            10,
        );
        let high = DockNode::vertical(
            DockNode::Tabs(DockTabs::new("a", vec![DockTab::new("a", "A", "A")])),
            DockNode::Tabs(DockTabs::new("b", vec![DockTab::new("b", "B", "B")])),
            990,
        );

        match low {
            DockNode::Split { axis, ratio, .. } => {
                assert_eq!(axis, DockAxis::Horizontal);
                assert_eq!(ratio, 150);
            }
            DockNode::Tabs(_) => unreachable!("expected split"),
        }

        match high {
            DockNode::Split { axis, ratio, .. } => {
                assert_eq!(axis, DockAxis::Vertical);
                assert_eq!(ratio, 850);
            }
            DockNode::Tabs(_) => unreachable!("expected split"),
        }
    }

    #[test]
    fn dock_widget_builds() {
        let dock = Dock::new("workspace-dock", sample_layout()).title("Workspace");
        assert_eq!(dock.title.as_deref(), Some("Workspace"));
    }

    #[gpui::test]
    fn dock_custom_tab_body_renderer_draws_active_body(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (_view, cx) = cx.add_window_view(|_, _| CustomDockBodyHarness);

        let bounds = cx
            .debug_bounds("guic-dock-custom-body-sidebar-files")
            .expect("custom active dock body should be rendered");
        assert!(bounds.size.width > gpui::px(0.0));
    }

    #[gpui::test]
    fn dock_tab_strip_scrolls_when_tabs_overflow(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let scroll_handle = ScrollHandle::new();
        let expected_handle = scroll_handle.clone();
        let (_view, cx) = cx.add_window_view(|_, _| OverflowDockHarness { scroll_handle });

        cx.debug_bounds("guic-dock-tab-strip-overflow")
            .expect("overflow tab strip should render");
        assert!(expected_handle.max_offset().x > px(0.0));
    }

    #[gpui::test]
    fn dock_tab_click_updates_active_tab(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        let tab_bounds = cx
            .debug_bounds("guic-dock-tab-sidebar-search")
            .expect("dock tab bounds should exist");
        cx.simulate_click(tab_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            let json = view.layout.to_json().expect("layout should serialize");
            assert!(json.contains("\"id\": \"search\""));
            assert!(json.contains("\"active_tab\": 1"));
        });
    }

    #[gpui::test]
    fn dock_keyboard_commands_select_and_move_tabs(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("ctrl-tab").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert_eq!(
                view.layout
                    .active_selection_in("sidebar")
                    .expect("sidebar has an active tab")
                    .tab_id()
                    .as_ref(),
                "search"
            );
        });

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("ctrl-shift-left").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert_eq!(
                view.layout.root.tab_index("sidebar", "search"),
                Some((0, 2))
            );
        });

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("ctrl-alt-right").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert_eq!(
                view.layout.focused_stack_id().map(AsRef::as_ref),
                Some("editor")
            );
        });

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("ctrl-alt-shift-down").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert!(view.layout.root.tab_index("console", "main").is_some());
            assert_eq!(
                view.layout.focused_stack_id().map(AsRef::as_ref),
                Some("console")
            );
        });

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("ctrl-shift-enter").expect("keystroke parses"),
        );
        view.update(cx, |view, _| {
            assert_eq!(view.layout.leaf_count(), 3);
            assert!(
                view.layout
                    .focused_stack_id()
                    .is_some_and(|id| id.contains("right"))
            );
        });
    }

    #[gpui::test]
    fn dock_tab_close_button_removes_tab(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        view.update(cx, |view, _| {
            assert_eq!(view.layout.tab_count(), 4);
        });

        let close_bounds = cx
            .debug_bounds("guic-dock-tab-close-sidebar-search")
            .expect("dock tab close affordance should exist");
        cx.simulate_click(close_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.layout.tab_count(), 3);
            let json = view.layout.to_json().expect("layout should serialize");
            assert!(!json.contains("\"id\": \"search\""));
            assert!(json.contains("\"id\": \"files\""));
        });
    }

    #[gpui::test]
    fn dock_tab_pin_button_updates_and_orders_tabs(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        let pin_bounds = cx
            .debug_bounds("guic-dock-tab-pin-sidebar-search")
            .expect("dock tab pin affordance should exist");
        cx.simulate_click(pin_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(
                view.layout.root.tab_index("sidebar", "search"),
                Some((0, 2))
            );
            let json = view.layout.to_json().expect("layout should serialize");
            assert!(json.contains("\"pinned\": true"));
        });
    }

    #[gpui::test]
    fn dock_stack_pin_button_protects_stack_close(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        let pin_bounds = cx
            .debug_bounds("guic-dock-stack-pin-editor")
            .expect("dock stack pin affordance should exist");
        cx.simulate_click(pin_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert!(!view.layout.close_stack("editor"));
            assert_eq!(view.layout.leaf_count(), 3);
        });
        assert!(cx.debug_bounds("guic-dock-stack-close-editor").is_none());
    }

    #[gpui::test]
    fn dock_stack_close_button_removes_stack(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        view.update(cx, |view, _| {
            assert_eq!(view.layout.leaf_count(), 3);
            assert_eq!(view.layout.tab_count(), 4);
        });

        let close_bounds = cx
            .debug_bounds("guic-dock-stack-close-console")
            .expect("dock stack close affordance should exist");
        cx.simulate_click(close_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.layout.leaf_count(), 2);
            assert_eq!(view.layout.tab_count(), 3);
            let json = view.layout.to_json().expect("layout should serialize");
            assert!(!json.contains("\"console\""));
            assert!(!json.contains("\"logs\""));
        });
    }

    #[gpui::test]
    fn dock_split_resize_handle_updates_layout(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        let handle_bounds = cx
            .debug_bounds("guic-dock-split-resize-sidebar")
            .expect("dock split resize affordance should exist");
        cx.simulate_click(handle_bounds.center(), Modifiers::none());

        view.update(cx, |view, _| {
            let json = view.layout.to_json().expect("layout should serialize");
            assert!(!json.contains("\"ratio\": 280"));
        });
    }

    #[gpui::test]
    fn dock_split_panes_remain_inside_the_dock_bounds(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (_view, cx) = cx.add_window_view(|_, cx| DockHarness::new(cx));

        let root = cx
            .debug_bounds("guic-dock-root-dock-harness")
            .expect("dock root bounds should exist");
        let sidebar = cx
            .debug_bounds("guic-dock-leaf-sidebar")
            .expect("sidebar bounds should exist");
        let editor = cx
            .debug_bounds("guic-dock-leaf-editor")
            .expect("editor bounds should exist");
        let console = cx
            .debug_bounds("guic-dock-leaf-console")
            .expect("console bounds should exist");
        let root_right = root.origin.x + root.size.width;
        let root_bottom = root.origin.y + root.size.height;

        for pane in [sidebar, editor, console] {
            assert!(pane.origin.x >= root.origin.x);
            assert!(pane.origin.y >= root.origin.y);
            assert!(pane.origin.x + pane.size.width <= root_right);
            assert!(pane.origin.y + pane.size.height <= root_bottom);
        }
        assert!(sidebar.origin.x + sidebar.size.width <= editor.origin.x);
        assert!(editor.origin.y + editor.size.height <= console.origin.y);
    }
}
