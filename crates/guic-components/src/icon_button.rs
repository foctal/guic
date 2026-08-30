use crate::{ButtonVariant, ClickHandler, ComponentSize};
use gpui::{
    App, ClickEvent, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, white,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// A square icon-only button.
#[derive(gpui::IntoElement)]
pub struct IconButton {
    icon: IconName,
    variant: ButtonVariant,
    size: ComponentSize,
    disabled: bool,
    label: Option<SharedString>,
    focus_handle: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    /// Creates a new icon button.
    #[must_use]
    pub fn new(icon: IconName) -> Self {
        Self {
            icon,
            variant: ButtonVariant::Ghost,
            size: ComponentSize::Medium,
            disabled: false,
            label: None,
            focus_handle: None,
            on_click: None,
        }
    }

    /// Sets the icon button variant.
    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the icon button size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the icon button is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets an accessibility label for icon-only button affordances.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets an application-owned focus handle for programmatic focus control.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a click handler.
    #[must_use]
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    pub(crate) fn on_click_option(mut self, on_click: Option<ClickHandler>) -> Self {
        self.on_click = on_click;
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let element_id = format!("guic-icon-button-{:?}", self.icon);
        let (dimension, icon_size) = match self.size {
            ComponentSize::Small => (px(28.0), 12.0),
            ComponentSize::Medium => (px(34.0), 14.0),
            ComponentSize::Large => (px(42.0), 16.0),
        };
        let (background, foreground, border) = match self.variant {
            ButtonVariant::Solid => (theme.secondary(), theme.foreground(), theme.border()),
            ButtonVariant::Primary => (theme.primary(), white(), theme.primary()),
            ButtonVariant::Secondary => (theme.background(), theme.foreground(), theme.border()),
            ButtonVariant::Ghost => (
                theme.background().opacity(0.01),
                theme.foreground(),
                theme.background().opacity(0.0),
            ),
            ButtonVariant::Danger => (theme.danger(), white(), theme.danger()),
        };

        let selector = element_id.clone();
        let accessibility_label = self
            .label
            .clone()
            .unwrap_or_else(|| format!("{:?}", self.icon).into());
        let hover_bg = background.opacity(if self.variant == ButtonVariant::Ghost {
            0.08
        } else {
            0.92
        });
        let button = div()
            .id(element_id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(accessibility_label)
                    .disabled(self.disabled),
            )
            .debug_selector(move || selector.clone())
            .size(dimension)
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(border)
            .bg(background)
            .text_color(foreground)
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(self.label.clone().map_or_else(
                || Icon::new(self.icon).size(icon_size).color(foreground),
                |label| {
                    Icon::new(self.icon)
                        .size(icon_size)
                        .color(foreground)
                        .label(label)
                },
            ));

        if self.disabled {
            button.opacity(0.45).cursor_default().into_any_element()
        } else if let Some(on_click) = self.on_click {
            let mut button = button
                .cursor_pointer()
                .hover(move |style: gpui::StyleRefinement| style.bg(hover_bg))
                .on_click(move |event, window, cx| (on_click)(event, window, cx));
            button = if let Some(focus_handle) = &self.focus_handle {
                button.track_focus(focus_handle)
            } else {
                button.tab_index(0)
            };
            button.into_any_element()
        } else {
            button.into_any_element()
        }
    }
}
