use crate::{BoolHandler, ComponentSize, IndexHandler};
use gpui::{
    App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// Immutable option metadata for [`Select`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectItem {
    /// Stable option identifier.
    pub id: SharedString,
    /// User-facing option label.
    pub label: SharedString,
    /// Whether the option is disabled.
    pub disabled: bool,
}

impl SelectItem {
    /// Creates a new option item.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Marks the option as disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Width policy for the floating options surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SelectMenuWidth {
    /// Exactly match the trigger width.
    #[default]
    MatchTrigger,
    /// Use the intrinsic option width, limited by the viewport.
    Content,
    /// Use an explicit width, limited by the viewport.
    Fixed(Pixels),
    /// Grow for content while remaining at least as wide as the trigger.
    MinTriggerWidth,
}

/// A controlled select component with a viewport-constrained overlay menu.
#[derive(gpui::IntoElement)]
pub struct Select {
    id: SharedString,
    width: Option<Pixels>,
    menu_width: SelectMenuWidth,
    max_menu_height: Pixels,
    exclusive_group: Option<SharedString>,
    items: Vec<SelectItem>,
    selected: Option<usize>,
    placeholder: SharedString,
    accessible_label: Option<SharedString>,
    empty_message: SharedString,
    expanded: bool,
    disabled: bool,
    size: ComponentSize,
    focus_handle: Option<FocusHandle>,
    on_toggle: Option<BoolHandler>,
    on_select: Option<IndexHandler>,
}

impl Select {
    /// Creates a new select component.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            width: None,
            menu_width: SelectMenuWidth::default(),
            max_menu_height: px(320.),
            exclusive_group: Some("guic-selects".into()),
            items: Vec::new(),
            selected: None,
            placeholder: "Select an option".into(),
            accessible_label: None,
            empty_message: "No options available".into(),
            expanded: false,
            disabled: false,
            size: ComponentSize::Medium,
            focus_handle: None,
            on_toggle: None,
            on_select: None,
        }
    }

    /// Sets a window-scoped exclusive group. Opening requests closure of its previous member.
    /// The default group is shared by all Selects. Pass None for independent menus.
    #[must_use]
    pub fn exclusive_group(mut self, group: Option<SharedString>) -> Self {
        self.exclusive_group = group;
        self
    }

    /// Sets the trigger width. The default fills available width.
    #[must_use]
    pub fn width(mut self, width: Pixels) -> Self {
        if f32::from(width).is_finite() {
            self.width = Some(width.max(px(1.)));
        }
        self
    }

    /// Sets the floating menu width policy.
    #[must_use]
    pub fn menu_width(mut self, policy: SelectMenuWidth) -> Self {
        if let SelectMenuWidth::Fixed(width) = policy {
            if !f32::from(width).is_finite() {
                return self;
            }
            self.menu_width = SelectMenuWidth::Fixed(width.max(px(1.)));
        } else {
            self.menu_width = policy;
        }
        self
    }

    /// Limits the menu height; overflowing options scroll internally.
    #[must_use]
    pub fn max_menu_height(mut self, height: Pixels) -> Self {
        if f32::from(height).is_finite() {
            self.max_menu_height = height.max(px(1.));
        }
        self
    }

    /// Replaces the option list.
    #[must_use]
    pub fn items(mut self, items: Vec<SelectItem>) -> Self {
        self.items = items;
        self
    }

    /// Sets the selected option index.
    #[must_use]
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the placeholder label.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the control name announced by assistive technologies.
    #[must_use]
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Sets the message rendered when the option list is empty.
    #[must_use]
    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }

    /// Sets the expanded state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the component size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets an application-owned focus handle for programmatic focus control.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle.tab_stop(true));
        self
    }

    /// Registers an expanded-state toggle handler.
    #[must_use]
    pub fn on_toggle(mut self, on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// Registers a selection handler.
    #[must_use]
    pub fn on_select(
        mut self,
        on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }
}

fn next_enabled_index(
    items: &[SelectItem],
    selected: Option<usize>,
    direction: isize,
) -> Option<usize> {
    if items.is_empty() || direction == 0 {
        return selected.filter(|index| items.get(*index).is_some_and(|item| !item.disabled));
    }

    if direction > 0 {
        let start = match selected {
            Some(index) => index.checked_add(1).unwrap_or(items.len()),
            None => 0,
        }
        .min(items.len());
        (start..items.len()).find(|index| !items[*index].disabled)
    } else {
        let start = match selected {
            Some(index) => index.checked_sub(1)?.min(items.len() - 1),
            None => items.len() - 1,
        };
        (0..=start).rev().find(|index| !items[*index].disabled)
    }
}

fn typeahead_index(items: &[SelectItem], selected: Option<usize>, query: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || items.is_empty() {
        return None;
    }
    let start = selected
        .and_then(|index| index.checked_add(1))
        .unwrap_or(0)
        .min(items.len());
    (start..items.len()).chain(0..start).find(|index| {
        let item = &items[*index];
        !item.disabled && item.label.to_lowercase().starts_with(&query)
    })
}

#[derive(Default)]
struct SelectScrollState {
    handle: gpui::ScrollHandle,
    member: guic_core::OverlayGroupMember,
    group: Option<SharedString>,
    open: bool,
    selected: Option<SharedString>,
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scroll = _window.use_keyed_state(format!("{}-scroll-state", self.id), cx, |_, _| {
            SelectScrollState::default()
        });
        let scroll_handle = scroll.update(cx, |state, cx| {
            let selected = self
                .selected
                .and_then(|index| self.items.get(index))
                .map(|item| item.id.clone());
            let open = self.expanded && !self.disabled;
            if state.open
                && (!open || state.group != self.exclusive_group)
                && let Some(group) = &state.group
            {
                state.member.deactivate(group, _window, cx);
            }
            if open
                && (!state.open || state.group != self.exclusive_group)
                && let (Some(group), Some(on_toggle)) =
                    (&self.exclusive_group, self.on_toggle.clone())
            {
                state
                    .member
                    .activate(group.clone(), _window, cx, move |window, cx| {
                        on_toggle(&false, window, cx)
                    });
            }
            state.group = self.exclusive_group.clone();
            if open
                && (!state.open || state.selected != selected)
                && let Some(index) = self.selected.filter(|index| *index < self.items.len())
            {
                let handle = state.handle.clone();
                _window.on_next_frame(move |window, _| {
                    handle.scroll_to_item(index);
                    window.refresh();
                });
            }
            state.open = open;
            state.selected = selected;
            state.handle.clone()
        });
        let theme = Theme::global(cx);
        let metrics = self.size.control_metrics(theme);
        let (height, text_size) = (metrics.height, metrics.font_size);
        let selected_label = self
            .selected
            .and_then(|index| self.items.get(index))
            .map(|item| item.label.clone())
            .unwrap_or_else(|| self.placeholder.clone());
        let accessible_label = self
            .accessible_label
            .clone()
            .unwrap_or_else(|| self.id.clone());

        let mut trigger = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(accessible_label)
                    .expanded(self.expanded)
                    .disabled(self.disabled),
            )
            .debug_selector(|| format!("guic-select-trigger-{}", self.id))
            .w_full()
            .h(height)
            .px(px(theme.spacing.x4))
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .text_size(text_size)
            .text_color(if self.selected.is_some() {
                theme.foreground()
            } else {
                theme.muted_foreground()
            })
            .flex()
            .items_center()
            .justify_between()
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(selected_label)
            .child(
                Icon::new(if self.expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .color(theme.muted_foreground()),
            );

        if let Some(width) = self.width {
            trigger = trigger.w(width);
        }

        let interactive = self.on_toggle.is_some() || self.on_select.is_some();
        if self.disabled {
            trigger = trigger.opacity(0.55);
        } else if interactive {
            let items = self.items.clone();
            let selected = self.selected;
            let expanded = self.expanded;
            let on_toggle = self.on_toggle.clone();
            let on_select = self.on_select.clone();
            trigger = trigger.key_context("GuicSelect").on_key_down(
                move |event: &KeyDownEvent, window, cx| {
                    let handled = match event.keystroke.key.as_str() {
                        "enter" | "space" => {
                            if let Some(handler) = on_toggle.as_ref() {
                                handler(&!expanded, window, cx);
                            }
                            true
                        }
                        "escape" if expanded => {
                            if let Some(handler) = on_toggle.as_ref() {
                                handler(&false, window, cx);
                            }
                            true
                        }
                        "down" | "up" => {
                            let direction = if event.keystroke.key == "down" { 1 } else { -1 };
                            if let (Some(index), Some(handler)) = (
                                next_enabled_index(&items, selected, direction),
                                on_select.as_ref(),
                            ) {
                                handler(&index, window, cx);
                            }
                            true
                        }
                        "home" | "end" => {
                            let direction = if event.keystroke.key == "home" { 1 } else { -1 };
                            if let (Some(index), Some(handler)) = (
                                next_enabled_index(&items, None, direction),
                                on_select.as_ref(),
                            ) {
                                handler(&index, window, cx);
                            }
                            true
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
                            if let (Some(index), Some(handler)) =
                                (typeahead_index(&items, selected, query), on_select.as_ref())
                            {
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
                },
            );

            trigger = if let Some(handle) = &self.focus_handle {
                trigger.track_focus(handle)
            } else {
                trigger.tab_index(0)
            };

            if let Some(on_toggle) = self.on_toggle.clone() {
                let next = !self.expanded;
                trigger = trigger
                    .cursor_pointer()
                    .hover({
                        let hover = theme.secondary().opacity(0.22);
                        move |style: gpui::StyleRefinement| style.bg(hover)
                    })
                    .on_click(move |_event: &ClickEvent, window, cx| {
                        (on_toggle)(&next, window, cx)
                    });
            }
        }
        let mut root = crate::behavioral_trigger::BehavioralTrigger::new(trigger);

        if self.expanded && !self.disabled {
            let mut menu = div()
                .id(format!("{}-menu", self.id))
                .accessibility(
                    AccessibilityProps::new(Role::ListBox).label(format!("{} options", self.id)),
                )
                .debug_selector(|| format!("guic-select-menu-{}", self.id))
                .overflow_y_scroll()
                .track_scroll(&scroll_handle)
                .rounded(px(theme.radius.md))
                .bg(theme.background())
                .shadow_lg()
                .flex()
                .flex_col();

            if self.items.is_empty() {
                menu = menu.child(
                    div()
                        .px(px(theme.spacing.x4))
                        .py(px(theme.spacing.x3))
                        .text_color(theme.muted_foreground())
                        .child(self.empty_message),
                );
            }

            for (index, item) in self.items.into_iter().enumerate() {
                let row = div()
                    .id(item.id.clone())
                    .accessibility(
                        AccessibilityProps::new(Role::Option)
                            .label(item.label.clone())
                            .selected(Some(index) == self.selected)
                            .disabled(item.disabled),
                    )
                    .debug_selector(|| format!("guic-select-item-{}", index))
                    .flex_shrink_0()
                    .px(px(theme.spacing.x4))
                    .py(px(theme.spacing.x3))
                    .text_color(if item.disabled {
                        theme.muted_foreground()
                    } else if Some(index) == self.selected {
                        theme.primary()
                    } else {
                        theme.foreground()
                    })
                    .bg(if Some(index) == self.selected {
                        theme.secondary().opacity(0.35)
                    } else {
                        theme.background()
                    })
                    .child(item.label);

                menu = if item.disabled {
                    menu.child(row.opacity(0.5))
                } else if let Some(on_select) = self.on_select.clone() {
                    let on_toggle = self.on_toggle.clone();
                    let focus = self.focus_handle.clone();
                    menu.child(
                        row.cursor_pointer()
                            .hover({
                                let hover = theme.secondary().opacity(0.24);
                                move |style: gpui::StyleRefinement| style.bg(hover)
                            })
                            .on_click(move |_event: &ClickEvent, window, cx| {
                                (on_select)(&index, window, cx);
                                if let Some(on_toggle) = &on_toggle {
                                    on_toggle(&false, window, cx);
                                }
                                if let Some(focus) = &focus {
                                    focus.focus(window, cx);
                                }
                            }),
                    )
                } else {
                    menu.child(row)
                };
            }

            if let Some(on_toggle) = self.on_toggle.clone() {
                menu = menu.on_mouse_down_out(move |_, window, cx| on_toggle(&false, window, cx));
            }
            root = root.anchored_overlay(move |bounds, window, _cx| {
                let viewport = window.viewport_size();
                let below = (viewport.height - bounds.bottom()).max(px(0.));
                let above = bounds.top().max(px(0.));
                let flip = below < self.max_menu_height && above > below;
                let available = if flip { above } else { below };
                menu = menu
                    .max_h(self.max_menu_height.min(available))
                    .max_w(viewport.width);
                menu = match self.menu_width {
                    SelectMenuWidth::MatchTrigger => menu.w(bounds.size.width.min(viewport.width)),
                    SelectMenuWidth::Content => menu,
                    SelectMenuWidth::Fixed(width) => menu.w(width.min(viewport.width)),
                    SelectMenuWidth::MinTriggerWidth => {
                        menu.min_w(bounds.size.width.min(viewport.width))
                    }
                };
                let (anchor, position) = if flip {
                    (gpui::Anchor::BottomLeft, bounds.origin)
                } else {
                    (gpui::Anchor::TopLeft, bounds.bottom_left())
                };
                overlay_portal(
                    gpui::anchored()
                        .anchor(anchor)
                        .position(position)
                        .child(menu),
                    OverlayPriority::FLOATING,
                )
            });
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::{SelectItem, next_enabled_index, typeahead_index};

    #[test]
    fn select_item_supports_disabled_state() {
        let item = SelectItem::new("alpha", "Alpha").disabled(true);
        assert!(item.disabled);
    }

    #[test]
    fn keyboard_navigation_skips_disabled_items() {
        let items = vec![
            SelectItem::new("a", "Alpha"),
            SelectItem::new("b", "Beta").disabled(true),
            SelectItem::new("c", "Charlie"),
        ];
        assert_eq!(next_enabled_index(&items, Some(0), 1), Some(2));
        assert_eq!(next_enabled_index(&items, Some(2), -1), Some(0));
        assert_eq!(next_enabled_index(&items, None, 1), Some(0));
        assert_eq!(next_enabled_index(&items, None, -1), Some(2));
        assert_eq!(next_enabled_index(&items, Some(2), 1), None);
        assert_eq!(next_enabled_index(&items, Some(0), -1), None);
        assert_eq!(next_enabled_index(&items, Some(usize::MAX), 1), None);
    }

    #[test]
    fn typeahead_wraps_and_skips_disabled_items() {
        let items = vec![
            SelectItem::new("alpha", "Alpha"),
            SelectItem::new("beta", "Beta").disabled(true),
            SelectItem::new("bravo", "Bravo"),
            SelectItem::new("charlie", "Charlie"),
        ];

        assert_eq!(typeahead_index(&items, None, "b"), Some(2));
        assert_eq!(typeahead_index(&items, Some(2), "a"), Some(0));
        assert_eq!(typeahead_index(&items, Some(0), "  CH"), Some(3));
        assert_eq!(typeahead_index(&items, Some(3), "missing"), None);
        assert_eq!(typeahead_index(&items, None, " "), None);
    }
}
