use crate::IndexHandler;
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// A single entry in a [`Breadcrumb`] trail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreadcrumbItem {
    /// Stable identifier used for the clickable region.
    pub id: SharedString,
    /// User-facing label.
    pub label: SharedString,
}

impl BreadcrumbItem {
    /// Creates a new breadcrumb item.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A horizontal navigation trail showing the path to the current location.
///
/// The final item is treated as the current page (non-interactive, emphasized).
/// Preceding items become clickable when [`Breadcrumb::on_select`] is set and
/// report their index.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Breadcrumb, BreadcrumbItem};
///
/// Breadcrumb::new("nav")
///     .items(vec![
///         BreadcrumbItem::new("home", "Home"),
///         BreadcrumbItem::new("settings", "Settings"),
///         BreadcrumbItem::new("profile", "Profile"),
///     ])
///     .on_select(|index, _, _| { /* navigate */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Breadcrumb {
    id: SharedString,
    items: Vec<BreadcrumbItem>,
    on_select: Option<IndexHandler>,
}

impl Breadcrumb {
    /// Creates a new, empty breadcrumb trail.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            on_select: None,
        }
    }

    /// Sets the breadcrumb items.
    #[must_use]
    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self
    }

    /// Registers a selection handler invoked with the selected item's index.
    /// The trailing (current) item never fires this callback.
    #[must_use]
    pub fn on_select(
        mut self,
        on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let last = self.items.len().saturating_sub(1);

        let mut row = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_1()
            .text_size(px(theme.typography.text_sm));

        for (ix, item) in self.items.into_iter().enumerate() {
            if ix > 0 {
                row = row.child(
                    Icon::new(IconName::ChevronRight)
                        .size(12.0)
                        .color(theme.muted_foreground())
                        .decorative(true),
                );
            }

            let is_current = ix == last;
            let crumb = div()
                .px_1()
                .text_color(if is_current {
                    theme.foreground()
                } else {
                    theme.muted_foreground()
                })
                .child(item.label.clone());

            row = if !is_current && self.on_select.is_some() {
                let on_select = self.on_select.clone();
                let selector = format!("guic-breadcrumb-{}", item.id);
                row.child(
                    crumb
                        .id(item.id)
                        .accessibility(AccessibilityProps::new(Role::Link).label(item.label))
                        .debug_selector(move || selector.clone())
                        .tab_index(0)
                        .key_context("GuicBreadcrumbLink")
                        .cursor_pointer()
                        .hover(|style: gpui::StyleRefinement| style.text_color(theme.primary()))
                        .on_key_down({
                            let on_select = on_select.clone();
                            move |event: &KeyDownEvent, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    if let Some(handler) = on_select.as_ref() {
                                        handler(&ix, window, cx);
                                    }
                                    cx.stop_propagation();
                                }
                            }
                        })
                        .on_click(move |_event: &ClickEvent, window, cx| {
                            if let Some(handler) = on_select.as_ref() {
                                handler(&ix, window, cx);
                            }
                        }),
                )
            } else {
                row.child(crumb)
            };
        }

        row
    }
}
