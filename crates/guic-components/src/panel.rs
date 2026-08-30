use crate::BoolHandler;
use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// A titled, optionally collapsible content region.
///
/// `Panel` is a lighter-weight container than [`Card`](crate::Card): it renders
/// a header bar with a title and an optional collapse toggle. Collapsing is
/// host-managed — provide [`Panel::collapsed`] and react to
/// [`Panel::on_toggle`].
///
/// # Example
///
/// ```no_run
/// use guic_components::{Label, Panel};
///
/// Panel::new("filters", "Filters")
///     .collapsible(true)
///     .collapsed(false)
///     .on_toggle(|collapsed, _, _| { /* persist */ })
///     .child(Label::new("Status: active"));
/// ```
#[derive(gpui::IntoElement)]
pub struct Panel {
    id: SharedString,
    title: SharedString,
    collapsible: bool,
    collapsed: bool,
    actions: Option<AnyElement>,
    body: Vec<AnyElement>,
    on_toggle: Option<BoolHandler>,
}

impl Panel {
    /// Creates a new panel with a stable id and a title.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            collapsible: false,
            collapsed: false,
            actions: None,
            body: Vec::new(),
            on_toggle: None,
        }
    }

    /// Enables the collapse toggle in the header.
    #[must_use]
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Sets whether the panel body is currently collapsed.
    #[must_use]
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Sets a trailing header element (for example, action buttons).
    #[must_use]
    pub fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    /// Appends a child element to the panel body.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Registers a collapse-toggle handler. The argument is the next collapsed
    /// state. Implies [`Panel::collapsible`].
    #[must_use]
    pub fn on_toggle(mut self, on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.collapsible = true;
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl RenderOnce for Panel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let collapsed = self.collapsible && self.collapsed;
        let collapsible = self.collapsible;
        let on_toggle = self.on_toggle.clone();
        let next_collapsed = !self.collapsed;
        let toggle_label = format!("Toggle {}", self.title);

        let header_id = SharedString::from(format!("{}-header", self.id));
        let selector = format!("guic-panel-header-{}", self.id);
        let mut header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px(px(theme.spacing.x4))
            .py(px(theme.spacing.x3));

        let mut heading = div()
            .id(header_id)
            .debug_selector(move || selector.clone())
            .flex()
            .items_center()
            .gap_2();
        if collapsible {
            heading = heading.child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .color(theme.muted_foreground())
                .decorative(true),
            );
        }
        heading = heading.child(
            div()
                .text_size(px(theme.typography.text_md))
                .text_color(theme.foreground())
                .child(self.title),
        );
        if collapsible {
            let keyboard_handler = on_toggle.clone();
            let click_handler = on_toggle.clone();
            heading = heading
                .accessibility(
                    AccessibilityProps::new(Role::Button)
                        .label(toggle_label)
                        .expanded(!collapsed),
                )
                .tab_index(0)
                .key_context("GuicPanelHeader")
                .cursor_pointer()
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        if let Some(handler) = keyboard_handler.as_ref() {
                            handler(&next_collapsed, window, cx);
                        }
                        cx.stop_propagation();
                    }
                })
                .on_click(move |_event: &ClickEvent, window, cx| {
                    if let Some(handler) = click_handler.as_ref() {
                        handler(&next_collapsed, window, cx);
                    }
                });
        }
        header = header.child(heading);
        if let Some(actions) = self.actions {
            header = header.child(actions);
        }

        let mut root = div()
            .id(self.id)
            .flex()
            .flex_col()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden()
            .child(header);

        if !collapsed {
            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(theme.spacing.x3))
                .px(px(theme.spacing.x4))
                .pb(px(theme.spacing.x4))
                .border_t_1()
                .border_color(theme.border())
                .pt(px(theme.spacing.x3));
            for child in self.body {
                body = body.child(child);
            }
            root = root.child(body);
        }

        root
    }
}
