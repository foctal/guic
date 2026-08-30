//! Menu surfaces: a reusable [`Menu`] panel, a top-level [`Menubar`], and a
//! right-click [`ContextMenu`].
//!
//! All three are *host-managed*: the open state lives in the host so that menus
//! compose cleanly with application command state, focus, and persistence. The
//! components emit intent (open/close/activate) and render the current state.

use gpui::{
    AnyElement, App, ClickEvent, Empty, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, point, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

type ActivateHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type HighlightHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;
type OpenIndexHandler = Rc<dyn Fn(&Option<usize>, &mut Window, &mut App)>;
type MenubarActivateHandler = Rc<dyn Fn(&MenubarActivation, &mut Window, &mut App)>;
type RequestHandler = Rc<dyn Fn(&Point<Pixels>, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The role of a [`MenuItem`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuItemKind {
    Action,
    Separator,
    Header,
}

/// A single entry within a [`Menu`].
///
/// Use [`MenuItem::new`] for an actionable command, [`MenuItem::separator`] for
/// a divider, and [`MenuItem::header`] for a non-interactive section label.
#[derive(Clone, Debug)]
pub struct MenuItem {
    id: SharedString,
    label: SharedString,
    kind: MenuItemKind,
    icon: Option<IconName>,
    shortcut: Option<SharedString>,
    children: Vec<MenuItem>,
    disabled: bool,
    danger: bool,
}

impl MenuItem {
    /// Creates an actionable menu item. The `id` is reported to `on_activate`.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: MenuItemKind::Action,
            icon: None,
            shortcut: None,
            children: Vec::new(),
            disabled: false,
            danger: false,
        }
    }

    /// Creates a non-interactive divider between groups of items.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            id: SharedString::default(),
            label: SharedString::default(),
            kind: MenuItemKind::Separator,
            icon: None,
            shortcut: None,
            children: Vec::new(),
            disabled: false,
            danger: false,
        }
    }

    /// Creates a non-interactive section header.
    #[must_use]
    pub fn header(label: impl Into<SharedString>) -> Self {
        Self {
            id: SharedString::default(),
            label: label.into(),
            kind: MenuItemKind::Header,
            icon: None,
            shortcut: None,
            children: Vec::new(),
            disabled: false,
            danger: false,
        }
    }

    /// Sets a leading icon.
    #[must_use]
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets a trailing keyboard-shortcut hint (for example, `"⌘S"`).
    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Sets child items for tiered menu presentation.
    #[must_use]
    pub fn children(mut self, children: Vec<MenuItem>) -> Self {
        self.children = children;
        self
    }

    /// Marks the item as disabled (not activatable).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Renders the item with destructive styling.
    #[must_use]
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Returns the item's id.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Returns whether the item can be activated by pointer or keyboard.
    #[must_use]
    pub fn is_activatable(&self) -> bool {
        self.kind == MenuItemKind::Action && !self.disabled
    }

    /// Returns whether this item has child menu entries.
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

fn first_menu_index(items: &[MenuItem]) -> Option<usize> {
    items.iter().position(MenuItem::is_activatable)
}

fn last_menu_index(items: &[MenuItem]) -> Option<usize> {
    items.iter().rposition(MenuItem::is_activatable)
}

fn next_menu_index(
    items: &[MenuItem],
    active_index: Option<usize>,
    direction: isize,
) -> Option<usize> {
    if items.is_empty() || direction == 0 {
        return None;
    }
    let start = active_index.filter(|index| *index < items.len());
    for offset in 1..=items.len() {
        let index = if direction > 0 {
            start.map_or(offset - 1, |index| (index + offset) % items.len())
        } else {
            start.map_or(items.len() - offset, |index| {
                (index + items.len() - (offset % items.len())) % items.len()
            })
        };
        if items[index].is_activatable() {
            return Some(index);
        }
    }
    None
}

fn menu_typeahead_index(
    items: &[MenuItem],
    active_index: Option<usize>,
    query: &str,
) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || items.is_empty() {
        return None;
    }
    let start = active_index
        .and_then(|index| index.checked_add(1))
        .unwrap_or(0)
        .min(items.len());
    (start..items.len()).chain(0..start).find(|index| {
        let item = &items[*index];
        item.is_activatable() && item.label.to_lowercase().starts_with(&query)
    })
}

/// Renders the body (list of rows) of a menu into `surface`.
fn render_menu_items(
    mut surface: gpui::Div,
    items: Vec<MenuItem>,
    id_prefix: &str,
    theme: &Theme,
    active_index: Option<usize>,
    on_activate: Option<ActivateHandler>,
) -> gpui::Div {
    for (index, item) in items.into_iter().enumerate() {
        match item.kind {
            MenuItemKind::Separator => {
                surface = surface.child(div().my_1().h(px(1.)).bg(theme.border()));
            }
            MenuItemKind::Header => {
                surface = surface.child(
                    div()
                        .px(px(theme.spacing.x3))
                        .pt(px(theme.spacing.x2))
                        .pb(px(theme.spacing.x1))
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(item.label),
                );
            }
            MenuItemKind::Action => {
                let active = active_index == Some(index);
                let foreground = if item.disabled {
                    theme.muted_foreground()
                } else if item.danger {
                    theme.danger()
                } else {
                    theme.foreground()
                };

                let mut leading = div().flex().items_center().gap_2();
                if let Some(icon) = item.icon {
                    leading = leading.child(Icon::new(icon).size(14.0).color(foreground));
                }
                leading = leading.child(item.label.clone());

                let mut row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .px(px(theme.spacing.x3))
                    .py(px(theme.spacing.x2))
                    .rounded(px(theme.radius.sm))
                    .bg(if active {
                        theme.secondary().opacity(0.4)
                    } else {
                        theme.background().opacity(0.0)
                    })
                    .text_size(px(theme.typography.text_md))
                    .text_color(foreground)
                    .child(leading);

                if let Some(shortcut) = item.shortcut {
                    row = row.child(
                        div()
                            .text_size(px(theme.typography.text_sm))
                            .text_color(theme.muted_foreground())
                            .child(shortcut),
                    );
                }

                surface = if item.disabled {
                    surface.child(row.opacity(0.55))
                } else if let Some(on_activate) = on_activate.clone() {
                    let id = item.id.clone();
                    let selector = format!("{id_prefix}-item-{index}");
                    let label = item.label.clone();
                    surface.child(
                        row.id(item.id)
                            .accessibility(
                                AccessibilityProps::new(Role::MenuItem)
                                    .label(label)
                                    .selected(active),
                            )
                            .debug_selector(move || selector.clone())
                            .cursor_pointer()
                            .hover({
                                let hover = theme.secondary().opacity(0.3);
                                move |style: gpui::StyleRefinement| style.bg(hover)
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                on_activate(&id, window, cx);
                            }),
                    )
                } else {
                    surface.child(row)
                };
            }
        }
    }
    surface
}

/// A reusable menu surface: a bordered panel of actionable rows, separators,
/// and section headers.
///
/// `Menu` is the building block shared by [`Menubar`] and [`ContextMenu`], but
/// it can also be embedded directly (for example, inside a [`crate::Popover`]).
///
/// # Example
///
/// ```no_run
/// use guic_components::{Menu, MenuItem};
/// use guic_icons::IconName;
///
/// Menu::new("file-menu")
///     .items(vec![
///         MenuItem::new("new", "New").icon(IconName::Plus).shortcut("⌘N"),
///         MenuItem::separator(),
///         MenuItem::new("delete", "Delete").danger(true),
///     ])
///     .on_activate(|id, _, _| { /* dispatch command */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Menu {
    id: SharedString,
    items: Vec<MenuItem>,
    min_width: Option<f32>,
    active_index: Option<usize>,
    focus_handle: Option<FocusHandle>,
    on_highlight: Option<HighlightHandler>,
    on_activate: Option<ActivateHandler>,
    on_close: Option<CloseHandler>,
}

/// A nested menu surface for hierarchical command groups.
///
/// `TieredMenu` renders [`MenuItem`] children recursively and reports
/// activation for any enabled action row. The host owns open/close state when
/// this surface is used inside a popover, drawer, or context menu.
#[derive(gpui::IntoElement)]
pub struct TieredMenu {
    id: SharedString,
    items: Vec<MenuItem>,
    min_width: Option<f32>,
    collapsed: Vec<SharedString>,
    on_activate: Option<ActivateHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl TieredMenu {
    /// Creates a new tiered menu.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            min_width: None,
            collapsed: Vec::new(),
            on_activate: None,
            on_toggle: None,
        }
    }

    /// Replaces the menu items.
    #[must_use]
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets a minimum surface width.
    #[must_use]
    pub fn min_width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.min_width = Some(width.max(120.0));
        }
        self
    }

    /// Sets branch item ids that should render collapsed.
    #[must_use]
    pub fn collapsed(mut self, collapsed: Vec<impl Into<SharedString>>) -> Self {
        self.collapsed = collapsed.into_iter().map(Into::into).collect();
        self
    }

    /// Registers an activation handler.
    #[must_use]
    pub fn on_activate(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a handler for branch expansion toggles.
    #[must_use]
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TieredMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let _menu_id = self.id;
        let mut surface = div()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_lg()
            .p_1()
            .flex()
            .flex_col()
            .gap_1();

        if let Some(width) = self.min_width {
            surface = surface.min_w(px(width));
        }

        render_tiered_menu_items(
            surface,
            self.items,
            0,
            &TieredMenuRenderContext {
                theme,
                collapsed: &self.collapsed,
                on_toggle: self.on_toggle,
                on_activate: self.on_activate,
            },
        )
    }
}

struct TieredMenuRenderContext<'a> {
    theme: &'a Theme,
    collapsed: &'a [SharedString],
    on_toggle: Option<ToggleHandler>,
    on_activate: Option<ActivateHandler>,
}

fn render_tiered_menu_items(
    mut surface: gpui::Div,
    items: Vec<MenuItem>,
    depth: usize,
    context: &TieredMenuRenderContext<'_>,
) -> gpui::Div {
    let theme = context.theme;
    for item in items {
        match item.kind {
            MenuItemKind::Separator => {
                surface = surface.child(div().my_1().h(px(1.)).bg(theme.border()));
            }
            MenuItemKind::Header => {
                surface = surface.child(
                    div()
                        .px(px(theme.spacing.x3 + depth as f32 * theme.spacing.x3))
                        .pt(px(theme.spacing.x2))
                        .pb(px(theme.spacing.x1))
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(item.label),
                );
            }
            MenuItemKind::Action => {
                let children = item.children.clone();
                let item_label = item.label.clone();
                let is_collapsed = context.collapsed.iter().any(|id| id == &item.id);
                let foreground = if item.disabled {
                    theme.muted_foreground()
                } else if item.danger {
                    theme.danger()
                } else {
                    theme.foreground()
                };
                let mut row = div()
                    .id(item.id.clone())
                    .accessibility(
                        AccessibilityProps::new(Role::MenuItem)
                            .label(item_label)
                            .expanded(!children.is_empty() && !is_collapsed),
                    )
                    .debug_selector({
                        let id = item.id.clone();
                        move || format!("guic-tiered-menu-item-{id}")
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px(px(theme.spacing.x3 + depth as f32 * theme.spacing.x3))
                    .py(px(theme.spacing.x2))
                    .rounded(px(theme.radius.sm))
                    .text_color(foreground);

                let mut leading = div().flex().items_center().gap_2();
                if let Some(icon) = item.icon {
                    leading = leading.child(Icon::new(icon).size(14.0).color(foreground));
                }
                leading = leading.child(item.label);
                row = row.child(leading);

                if children.is_empty() {
                    if let Some(shortcut) = item.shortcut {
                        row = row.child(
                            div()
                                .text_size(px(theme.typography.text_sm))
                                .text_color(theme.muted_foreground())
                                .child(shortcut),
                        );
                    }
                } else {
                    row = row.child(
                        Icon::new(if is_collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .size(14.0)
                        .color(foreground),
                    );
                }

                surface = if item.disabled {
                    surface.child(row.opacity(0.55))
                } else if children.is_empty() {
                    if let Some(on_activate) = context.on_activate.clone() {
                        let id = item.id.clone();
                        surface.child(row.cursor_pointer().on_click(
                            move |_event: &ClickEvent, window, cx| {
                                on_activate(&id, window, cx);
                            },
                        ))
                    } else {
                        surface.child(row)
                    }
                } else {
                    let branch_id = item.id.clone();
                    let mut branch = row;
                    if let Some(on_toggle) = context.on_toggle.clone() {
                        branch = branch.cursor_pointer().on_click(
                            move |_event: &ClickEvent, window, cx| {
                                on_toggle(&branch_id, window, cx);
                            },
                        );
                    }
                    surface = surface.child(branch);
                    if !is_collapsed {
                        surface = surface.child(render_tiered_menu_items(
                            div().flex().flex_col().gap_1(),
                            children,
                            depth + 1,
                            context,
                        ));
                    }
                    surface
                };
            }
        }
    }
    surface
}

impl Menu {
    /// Creates a new, empty menu.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            min_width: None,
            active_index: None,
            focus_handle: None,
            on_highlight: None,
            on_activate: None,
            on_close: None,
        }
    }

    /// Sets the menu items.
    #[must_use]
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets a minimum panel width in pixels.
    #[must_use]
    pub fn min_width(mut self, min_width: f32) -> Self {
        if min_width.is_finite() {
            self.min_width = Some(min_width.max(1.0));
        }
        self
    }

    /// Sets the host-managed active row used by keyboard navigation.
    ///
    /// The index addresses the complete item slice, including headers and
    /// separators. Non-action or disabled indices render no active row.
    #[must_use]
    pub fn active_index(mut self, active_index: Option<usize>) -> Self {
        self.active_index = active_index;
        self
    }

    /// Makes the menu keyboard-focusable so `Escape` dismisses it via
    /// [`Menu::on_close`]. The host owns the [`FocusHandle`].
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers an activation handler invoked with the activated item's id.
    #[must_use]
    pub fn on_activate(
        mut self,
        on_activate: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(on_activate));
        self
    }

    /// Registers a handler for keyboard-driven active-row changes.
    #[must_use]
    pub fn on_highlight(
        mut self,
        on_highlight: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_highlight = Some(Rc::new(on_highlight));
        self
    }

    /// Registers a close handler fired when the user presses `Escape` while the
    /// focusable menu holds focus.
    #[must_use]
    pub fn on_close(mut self, on_close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }

    fn into_surface(self, theme: &Theme) -> gpui::Stateful<gpui::Div> {
        let items = self.items;
        let active_index = self
            .active_index
            .filter(|index| items.get(*index).is_some_and(MenuItem::is_activatable));
        let mut surface = div()
            .id(self.id.clone())
            .accessibility(AccessibilityProps::new(Role::Menu))
            .flex()
            .flex_col()
            .p_1()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .shadow_lg();

        if let Some(min_width) = self.min_width {
            surface = surface.min_w(px(min_width));
        }

        if let Some(handle) = &self.focus_handle {
            surface = surface.key_context("GuicMenu").track_focus(handle);
            let key_items = items.clone();
            let on_close = self.on_close.clone();
            let on_highlight = self.on_highlight.clone();
            let on_activate = self.on_activate.clone();
            surface = surface.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let handled = match event.keystroke.key.as_str() {
                    "escape" => {
                        if let Some(handler) = on_close.as_ref() {
                            handler(window, cx);
                            true
                        } else {
                            false
                        }
                    }
                    "down" | "up" | "home" | "end" => {
                        let next = match event.keystroke.key.as_str() {
                            "down" => next_menu_index(&key_items, active_index, 1),
                            "up" => next_menu_index(&key_items, active_index, -1),
                            "home" => first_menu_index(&key_items),
                            "end" => last_menu_index(&key_items),
                            _ => None,
                        };
                        if let (Some(index), Some(handler)) = (next, on_highlight.as_ref()) {
                            handler(&index, window, cx);
                            true
                        } else {
                            false
                        }
                    }
                    "enter" | "space" => {
                        if let (Some(item), Some(handler)) = (
                            active_index.and_then(|index| key_items.get(index)),
                            on_activate.as_ref(),
                        ) {
                            handler(item.id(), window, cx);
                            true
                        } else {
                            false
                        }
                    }
                    _ if !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.platform =>
                    {
                        let query = event
                            .keystroke
                            .key_char
                            .as_deref()
                            .unwrap_or(&event.keystroke.key);
                        if let (Some(index), Some(handler)) = (
                            menu_typeahead_index(&key_items, active_index, query),
                            on_highlight.as_ref(),
                        ) {
                            handler(&index, window, cx);
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

        let id_prefix = format!("guic-menu-{}", self.id);
        let body = render_menu_items(
            div().flex().flex_col(),
            items,
            &id_prefix,
            theme,
            active_index,
            self.on_activate,
        );
        surface.child(body)
    }
}

impl RenderOnce for Menu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        self.into_surface(theme)
    }
}

/// A top-level menu within a [`Menubar`].
#[derive(Clone, Debug)]
pub struct MenubarMenu {
    label: SharedString,
    items: Vec<MenuItem>,
}

impl MenubarMenu {
    /// Creates a new top-level menu with a label and its items.
    #[must_use]
    pub fn new(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

/// The payload reported when a [`Menubar`] item is activated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenubarActivation {
    /// Index of the top-level menu that owns the item.
    pub menu: usize,
    /// The activated item's id.
    pub item: SharedString,
}

/// A horizontal application menu bar with host-managed open state.
///
/// Clicking a top-level label toggles its dropdown; the host owns which menu is
/// open via [`Menubar::open`] and reacts to [`Menubar::on_open`]. Item
/// activations are reported through [`Menubar::on_activate`].
///
/// # Example
///
/// ```no_run
/// use guic_components::{Menubar, MenubarMenu, MenuItem};
///
/// Menubar::new("app-menubar")
///     .menus(vec![
///         MenubarMenu::new("File", vec![MenuItem::new("open", "Open")]),
///         MenubarMenu::new("Edit", vec![MenuItem::new("undo", "Undo")]),
///     ])
///     .open(None)
///     .on_open(|next, _, _| { /* store open index */ })
///     .on_activate(|activation, _, _| { /* dispatch */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Menubar {
    id: SharedString,
    menus: Vec<MenubarMenu>,
    open: Option<usize>,
    on_open: Option<OpenIndexHandler>,
    on_activate: Option<MenubarActivateHandler>,
}

impl Menubar {
    /// Creates a new, empty menu bar.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            menus: Vec::new(),
            open: None,
            on_open: None,
            on_activate: None,
        }
    }

    /// Sets the top-level menus.
    #[must_use]
    pub fn menus(mut self, menus: Vec<MenubarMenu>) -> Self {
        self.menus = menus;
        self
    }

    /// Sets which top-level menu is currently open.
    #[must_use]
    pub fn open(mut self, open: Option<usize>) -> Self {
        self.open = open;
        self
    }

    /// Registers a handler invoked with the next open index (or `None` to close).
    #[must_use]
    pub fn on_open(
        mut self,
        on_open: impl Fn(&Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(on_open));
        self
    }

    /// Registers a handler invoked when a menu item is activated.
    #[must_use]
    pub fn on_activate(
        mut self,
        on_activate: impl Fn(&MenubarActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(on_activate));
        self
    }
}

impl RenderOnce for Menubar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut bar = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background());

        for (index, menu) in self.menus.into_iter().enumerate() {
            let is_open = self.open == Some(index);
            let label_selector = format!("guic-menubar-{}-label-{index}", self.id);

            let mut label = div()
                .id(SharedString::from(format!("{}-label-{index}", self.id)))
                .debug_selector(move || label_selector.clone())
                .px(px(theme.spacing.x3))
                .py(px(theme.spacing.x1_5))
                .rounded(px(theme.radius.sm))
                .text_size(px(theme.typography.text_md))
                .text_color(theme.foreground())
                .cursor_pointer()
                .bg(if is_open {
                    theme.secondary().opacity(0.4)
                } else {
                    theme.background().opacity(0.0)
                })
                .hover({
                    let hover = theme.secondary().opacity(0.3);
                    move |style: gpui::StyleRefinement| style.bg(hover)
                })
                .child(menu.label);

            if let Some(on_open) = self.on_open.clone() {
                let next = if is_open { None } else { Some(index) };
                label = label.on_click(move |_event: &ClickEvent, window, cx| {
                    on_open(&next, window, cx);
                });
            }

            let mut anchor = div().relative().flex().flex_col().child(label);

            if is_open {
                let on_activate = self.on_activate.clone();
                let panel_id = format!("{}-panel-{index}", self.id);
                let surface = Menu::new(panel_id)
                    .items(menu.items)
                    .min_width(180.0)
                    .on_activate(move |item, window, cx| {
                        if let Some(handler) = on_activate.as_ref() {
                            handler(
                                &MenubarActivation {
                                    menu: index,
                                    item: item.clone(),
                                },
                                window,
                                cx,
                            );
                        }
                    });

                anchor = anchor.child(overlay_portal(
                    div().absolute().top_full().left_0().mt_1().child(surface),
                    OverlayPriority::FLOATING,
                ));
            }

            bar = bar.child(anchor);
        }

        bar
    }
}

/// A right-click context menu attached to a trigger element.
///
/// `ContextMenu` is host-managed: a secondary (right) mouse press on the
/// trigger fires [`ContextMenu::on_request`] with the pointer position; the
/// host stores the position and open flag and passes them back via
/// [`ContextMenu::open`] / [`ContextMenu::anchor`]. While open, a full-window
/// scrim closes the menu on outside click.
///
/// # Example
///
/// ```no_run
/// use guic_components::{ContextMenu, MenuItem, Label};
///
/// ContextMenu::new("row-context", Label::new("Right-click me"))
///     .items(vec![MenuItem::new("rename", "Rename")])
///     .open(false)
///     .on_request(|position, _, _| { /* store position + open */ })
///     .on_activate(|id, _, _| { /* dispatch */ })
///     .on_close(|_, _| { /* close */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct ContextMenu {
    id: SharedString,
    trigger: AnyElement,
    items: Vec<MenuItem>,
    open: bool,
    anchor: Point<Pixels>,
    focus_handle: Option<FocusHandle>,
    on_request: Option<RequestHandler>,
    on_activate: Option<ActivateHandler>,
    on_close: Option<CloseHandler>,
}

impl ContextMenu {
    /// Creates a new context menu wrapping a trigger element.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, trigger: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            items: Vec::new(),
            open: false,
            anchor: point(px(0.0), px(0.0)),
            focus_handle: None,
            on_request: None,
            on_activate: None,
            on_close: None,
        }
    }

    /// Sets the menu items.
    #[must_use]
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets whether the menu is currently open.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Sets the window-space anchor position for the open menu.
    #[must_use]
    pub fn anchor(mut self, anchor: Point<Pixels>) -> Self {
        self.anchor = anchor;
        self
    }

    /// Makes the open menu keyboard-focusable so `Escape` closes it.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a handler fired on secondary (right) press with the position.
    #[must_use]
    pub fn on_request(
        mut self,
        on_request: impl Fn(&Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_request = Some(Rc::new(on_request));
        self
    }

    /// Registers an activation handler invoked with the activated item's id.
    #[must_use]
    pub fn on_activate(
        mut self,
        on_activate: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(on_activate));
        self
    }

    /// Registers a close handler fired by the scrim or `Escape`.
    #[must_use]
    pub fn on_close(mut self, on_close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let trigger_selector = format!("guic-context-menu-trigger-{}", self.id);

        let mut trigger = div()
            .id(SharedString::from(format!("{}-trigger", self.id)))
            .debug_selector(move || trigger_selector.clone())
            .child(self.trigger);

        if let Some(on_request) = self.on_request.clone() {
            trigger = trigger.on_mouse_down(
                MouseButton::Right,
                move |event: &MouseDownEvent, window, cx| {
                    on_request(&event.position, window, cx);
                },
            );
        }

        let mut root = div().relative().child(trigger);

        if self.open {
            let scrim_selector = format!("guic-context-menu-scrim-{}", self.id);
            let mut scrim = div()
                .id(SharedString::from(format!("{}-scrim", self.id)))
                .debug_selector(move || scrim_selector.clone())
                .absolute()
                .inset_0();
            if let Some(on_close) = self.on_close.clone() {
                scrim = scrim.on_click(move |_event: &ClickEvent, window, cx| {
                    on_close(window, cx);
                });
            }

            let mut menu = Menu::new(format!("{}-menu", self.id))
                .items(self.items)
                .min_width(180.0);
            if let Some(on_activate) = self.on_activate.clone() {
                menu = menu.on_activate(move |id, window, cx| on_activate(id, window, cx));
            }
            if let Some(handle) = self.focus_handle.clone() {
                menu = menu.focusable(handle);
                if let Some(on_close) = self.on_close.clone() {
                    menu = menu.on_close(move |window, cx| on_close(window, cx));
                }
            }

            root = root.child(overlay_portal(
                div().absolute().inset_0().child(scrim).child(
                    div()
                        .absolute()
                        .left(self.anchor.x)
                        .top(self.anchor.y)
                        .child(menu),
                ),
                OverlayPriority::MODAL,
            ));
        } else {
            root = root.child(Empty);
        }

        root
    }
}

/// A controlled vertical navigation menu suitable for persistent side panels.
///
/// Unlike [`Menu`], a `PanelMenu` keeps its selected entry visible and does not
/// represent a transient overlay. The application owns selection and updates it
/// in response to [`PanelMenu::on_select`].
#[derive(gpui::IntoElement)]
pub struct PanelMenu {
    id: SharedString,
    items: Vec<MenuItem>,
    selected: Option<SharedString>,
    on_select: Option<ActivateHandler>,
}

impl PanelMenu {
    /// Creates an empty panel menu.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            on_select: None,
        }
    }
    /// Sets the displayed menu items. Headers and separators are supported.
    #[must_use]
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }
    /// Sets the selected action id, or clears selection with `None`.
    #[must_use]
    pub fn selected(mut self, selected: Option<impl Into<SharedString>>) -> Self {
        self.selected = selected.map(Into::into);
        self
    }
    /// Registers a handler for an activated action id.
    #[must_use]
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PanelMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let selected = self.selected;
        let mut surface = div()
            .id(self.id)
            .w_full()
            .p_2()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .flex()
            .flex_col()
            .gap_1();
        for (index, item) in self.items.into_iter().enumerate() {
            match item.kind {
                MenuItemKind::Separator => {
                    surface = surface.child(div().my_1().h(px(1.)).bg(theme.border()))
                }
                MenuItemKind::Header => {
                    surface = surface.child(
                        div()
                            .px_2()
                            .pt_2()
                            .text_size(px(theme.typography.text_sm))
                            .text_color(theme.muted_foreground())
                            .child(item.label),
                    )
                }
                MenuItemKind::Action => {
                    let active = selected.as_ref() == Some(&item.id);
                    let foreground = if item.disabled {
                        theme.muted_foreground()
                    } else if item.danger {
                        theme.danger()
                    } else if active {
                        theme.primary()
                    } else {
                        theme.foreground()
                    };
                    let mut leading = div().flex().items_center().gap_2();
                    if let Some(icon) = item.icon {
                        leading = leading.child(Icon::new(icon).size(14.).color(foreground));
                    }
                    leading = leading.child(item.label);
                    let row = div()
                        .id(item.id.clone())
                        .debug_selector({
                            let selector = format!("guic-panel-menu-item-{index}");
                            move || selector.clone()
                        })
                        .px_2()
                        .py(px(theme.spacing.x2))
                        .rounded(px(theme.radius.sm))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_color(foreground)
                        .bg(if active {
                            theme.primary().opacity(0.12)
                        } else {
                            theme.background().opacity(0.)
                        })
                        .child(leading);
                    surface = if item.disabled {
                        surface.child(row.opacity(0.55))
                    } else if let Some(handler) = self.on_select.clone() {
                        let id = item.id;
                        surface.child(
                            row.cursor_pointer()
                                .hover({
                                    let hover = theme.secondary().opacity(0.3);
                                    move |style: gpui::StyleRefinement| style.bg(hover)
                                })
                                .on_click(move |_, window, cx| handler(&id, window, cx)),
                        )
                    } else {
                        surface.child(row)
                    };
                }
            }
        }
        surface
    }
}

#[cfg(test)]
mod tests {
    use super::{Menu, MenuItem, MenubarActivation, PanelMenu, TieredMenu};

    #[test]
    fn action_item_is_activatable() {
        assert!(MenuItem::new("open", "Open").is_activatable());
    }

    #[test]
    fn disabled_action_is_not_activatable() {
        assert!(
            !MenuItem::new("open", "Open")
                .disabled(true)
                .is_activatable()
        );
    }

    #[test]
    fn menu_widths_reject_non_finite_values() {
        assert_eq!(Menu::new("menu").min_width(f32::INFINITY).min_width, None);
        assert_eq!(
            TieredMenu::new("tiered").min_width(f32::NAN).min_width,
            None
        );
    }

    #[test]
    fn separators_and_headers_are_not_activatable() {
        assert!(!MenuItem::separator().is_activatable());
        assert!(!MenuItem::header("Section").is_activatable());
    }

    #[test]
    fn panel_menu_builder_tracks_selection() {
        let menu = PanelMenu::new("navigation")
            .items(vec![MenuItem::new("overview", "Overview")])
            .selected(Some("overview"));
        assert_eq!(menu.selected.as_deref(), Some("overview"));
        assert_eq!(menu.items.len(), 1);
    }

    #[test]
    fn tiered_menu_items_track_children() {
        let item = MenuItem::new("new", "New").children(vec![MenuItem::new("file", "File")]);
        assert!(item.has_children());

        let menu = TieredMenu::new("create").items(vec![item]).min_width(180.0);
        assert_eq!(menu.items.len(), 1);
        assert_eq!(menu.min_width, Some(180.0));
    }

    #[test]
    fn menubar_activation_round_trips() {
        let activation = MenubarActivation {
            menu: 1,
            item: "save".into(),
        };
        assert_eq!(activation.menu, 1);
        assert_eq!(activation.item.as_ref(), "save");
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::{ContextMenu, Menu, MenuItem, Menubar, MenubarActivation, MenubarMenu, PanelMenu};
    use gpui::{
        AppContext as _, Context, FocusHandle, Keystroke, Modifiers, MouseButton,
        ParentElement as _, Render, SharedString, Styled as _, TestAppContext, VisualContext as _,
        Window, div, point, px,
    };

    struct MenuHarness {
        menu_activated: Option<SharedString>,
        menu_active: Option<usize>,
        menu_closed: bool,
        menubar_open: Option<usize>,
        menubar_activated: Option<MenubarActivation>,
        context_open: bool,
        context_requested: bool,
        context_activated: Option<SharedString>,
        context_closed: bool,
        panel_selected: Option<SharedString>,
        menu_focus: FocusHandle,
        context_focus: FocusHandle,
    }

    impl MenuHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                menu_activated: None,
                menu_active: None,
                menu_closed: false,
                menubar_open: None,
                menubar_activated: None,
                context_open: false,
                context_requested: false,
                context_activated: None,
                context_closed: false,
                panel_selected: None,
                menu_focus: cx.focus_handle(),
                context_focus: cx.focus_handle(),
            }
        }
    }

    impl Render for MenuHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            let menu_close_view = cx.entity();
            let context_close_view = cx.entity();
            div()
                .size_full()
                .p_4()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    Menu::new("test-menu")
                        .focusable(self.menu_focus.clone())
                        .items(vec![
                            MenuItem::new("new", "New"),
                            MenuItem::separator(),
                            MenuItem::new("delete", "Delete").danger(true),
                        ])
                        .active_index(self.menu_active)
                        .on_highlight(cx.listener(|this, index: &usize, _, cx| {
                            this.menu_active = Some(*index);
                            cx.notify();
                        }))
                        .on_activate(cx.listener(|this, id: &SharedString, _, cx| {
                            this.menu_activated = Some(id.clone());
                            cx.notify();
                        }))
                        .on_close(move |_window, app| {
                            menu_close_view.update(app, |this, cx| {
                                this.menu_closed = true;
                                cx.notify();
                            });
                        }),
                )
                .child(
                    PanelMenu::new("test-panel-menu")
                        .items(vec![
                            MenuItem::new("overview", "Overview"),
                            MenuItem::new("disabled", "Disabled").disabled(true),
                        ])
                        .selected(self.panel_selected.clone())
                        .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                            this.panel_selected = Some(id.clone());
                            cx.notify();
                        })),
                )
                .child(
                    Menubar::new("app-bar")
                        .open(self.menubar_open)
                        .menus(vec![
                            MenubarMenu::new(
                                "File",
                                vec![MenuItem::new("open", "Open"), MenuItem::new("save", "Save")],
                            ),
                            MenubarMenu::new("Edit", vec![MenuItem::new("undo", "Undo")]),
                        ])
                        .on_open(cx.listener(|this, next: &Option<usize>, _, cx| {
                            this.menubar_open = *next;
                            cx.notify();
                        }))
                        .on_activate(cx.listener(|this, activation: &MenubarActivation, _, cx| {
                            this.menubar_activated = Some(activation.clone());
                            this.menubar_open = None;
                            cx.notify();
                        })),
                )
                .child(
                    ContextMenu::new("ctx", div().w(px(80.)).h(px(24.)).child("target"))
                        .focusable(self.context_focus.clone())
                        .open(self.context_open)
                        .anchor(point(px(10.), px(10.)))
                        .items(vec![MenuItem::new("rename", "Rename")])
                        .on_request(cx.listener(|this, _pos, _, cx| {
                            this.context_requested = true;
                            this.context_open = true;
                            cx.notify();
                        }))
                        .on_activate(cx.listener(|this, id: &SharedString, _, cx| {
                            this.context_activated = Some(id.clone());
                            this.context_open = false;
                            cx.notify();
                        }))
                        .on_close(move |_window, app| {
                            context_close_view.update(app, |this, cx| {
                                this.context_closed = true;
                                this.context_open = false;
                                cx.notify();
                            });
                        }),
                )
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn menu_item_click_reports_activation(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));

        let item = cx
            .debug_bounds("guic-menu-test-menu-item-0")
            .expect("first menu item should be present");
        cx.simulate_click(item.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.menu_activated.as_deref(), Some("new"));
        });
    }

    #[gpui::test]
    fn panel_menu_click_reports_selected_action(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));
        let item = cx
            .debug_bounds("guic-panel-menu-item-0")
            .expect("panel menu item should be present");
        cx.simulate_click(item.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(view.panel_selected.as_deref(), Some("overview"));
        });
    }

    #[gpui::test]
    fn menu_escape_closes_focused_surface(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.menu_focus.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(
            window,
            Keystroke::parse("escape").expect("keystroke parses"),
        );

        view.update(cx, |view, _| assert!(view.menu_closed));
    }

    #[gpui::test]
    fn menu_keyboard_navigation_skips_non_actions_and_activates(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.menu_focus.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.menu_active, Some(0)));

        cx.dispatch_keystroke(window, Keystroke::parse("down").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.menu_active, Some(2)));

        cx.dispatch_keystroke(window, Keystroke::parse("enter").expect("keystroke parses"));
        view.update(cx, |view, _| {
            assert_eq!(view.menu_activated.as_deref(), Some("delete"));
        });
    }

    #[gpui::test]
    fn menu_home_end_and_typeahead_update_active_item(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.menu_focus.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("end").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.menu_active, Some(2)));

        cx.dispatch_keystroke(window, Keystroke::parse("n").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.menu_active, Some(0)));

        cx.dispatch_keystroke(window, Keystroke::parse("home").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.menu_active, Some(0)));
    }

    #[gpui::test]
    fn menubar_label_click_toggles_open_then_activates(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));

        let label = cx
            .debug_bounds("guic-menubar-app-bar-label-0")
            .expect("first menubar label should be present");
        cx.simulate_click(label.center(), Modifiers::none());

        view.update(cx, |view, _| assert_eq!(view.menubar_open, Some(0)));

        let item = cx
            .debug_bounds("guic-menu-app-bar-panel-0-item-1")
            .expect("open menubar dropdown item should be present");
        cx.simulate_click(item.center(), Modifiers::none());

        view.update(cx, |view, _| {
            let activation = view.menubar_activated.clone().expect("activation recorded");
            assert_eq!(activation.menu, 0);
            assert_eq!(activation.item.as_ref(), "save");
            assert_eq!(view.menubar_open, None);
        });
    }

    #[gpui::test]
    fn context_menu_right_click_requests_then_activates(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| MenuHarness::new(cx));

        let trigger = cx
            .debug_bounds("guic-context-menu-trigger-ctx")
            .expect("context menu trigger should be present");
        cx.simulate_mouse_down(trigger.center(), MouseButton::Right, Modifiers::none());

        view.update(cx, |view, _| {
            assert!(view.context_requested);
            assert!(view.context_open);
        });

        let item = cx
            .debug_bounds("guic-menu-ctx-menu-item-0")
            .expect("context menu item should be present");
        cx.simulate_click(item.center(), Modifiers::none());

        view.update(cx, |view, _| {
            assert_eq!(view.context_activated.as_deref(), Some("rename"));
            assert!(!view.context_open);
        });
    }
}
