use crate::{ClickHandler, ComponentSize};
use gpui::{
    App, ClickEvent, FocusHandle, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, white,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// Supported button variants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
    /// Neutral surface button.
    #[default]
    Solid,
    /// Prominent primary action.
    Primary,
    /// Secondary outlined action.
    Secondary,
    /// Low-emphasis ghost action.
    Ghost,
    /// Destructive action.
    Danger,
}

/// A clickable GUIC button.
#[derive(gpui::IntoElement)]
pub struct Button {
    label: SharedString,
    variant: ButtonVariant,
    size: ComponentSize,
    disabled: bool,
    full_width: bool,
    focus_handle: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl Button {
    /// Creates a new button.
    #[must_use]
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Solid,
            size: ComponentSize::Medium,
            disabled: false,
            full_width: false,
            focus_handle: None,
            on_click: None,
        }
    }

    /// Sets the button variant.
    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Applies the primary button variant.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    /// Applies the secondary button variant.
    #[must_use]
    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    /// Applies the ghost button variant.
    #[must_use]
    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    /// Applies the danger button variant.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }

    /// Sets the button size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the button is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes the button fill the available width.
    #[must_use]
    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    /// Sets an application-owned focus handle for programmatic focus control.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a click handler for the button.
    #[must_use]
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    fn palette(&self, theme: &Theme) -> ButtonPalette {
        match self.variant {
            ButtonVariant::Solid => ButtonPalette {
                background: theme.secondary(),
                foreground: theme.foreground(),
                border: theme.border(),
            },
            ButtonVariant::Primary => ButtonPalette {
                background: theme.primary(),
                foreground: white(),
                border: theme.primary(),
            },
            ButtonVariant::Secondary => ButtonPalette {
                background: theme.background(),
                foreground: theme.foreground(),
                border: theme.border(),
            },
            ButtonVariant::Ghost => ButtonPalette {
                background: theme.background().opacity(0.01),
                foreground: theme.foreground(),
                border: theme.background().opacity(0.0),
            },
            ButtonVariant::Danger => ButtonPalette {
                background: theme.danger(),
                foreground: white(),
                border: theme.danger(),
            },
        }
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let palette = self.palette(theme);
        let element_id = format!("guic-button-{}", self.label);
        let hover_bg = palette
            .background
            .opacity(if self.variant == ButtonVariant::Ghost {
                0.08
            } else {
                0.92
            });
        let (height, padding_x, text_size) = match self.size {
            ComponentSize::Small => (px(28.0), px(theme.spacing.x3), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(34.0), px(theme.spacing.x4), px(theme.typography.text_md)),
            ComponentSize::Large => (px(42.0), px(theme.spacing.x5), px(theme.typography.text_lg)),
        };

        let button_label = self.label.clone();
        let mut button = div()
            .id(element_id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(button_label)
                    .disabled(self.disabled),
            )
            .debug_selector(|| element_id.clone())
            .h(height)
            .px(padding_x)
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(palette.border)
            .bg(palette.background)
            .text_color(palette.foreground)
            .text_size(text_size)
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(self.label);

        if self.full_width {
            button = button.w_full();
        }

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

#[derive(Clone, Copy)]
struct ButtonPalette {
    background: Hsla,
    foreground: Hsla,
    border: Hsla,
}

#[cfg(test)]
mod tests {
    use super::{Button, ButtonVariant};
    use guic_tokens::Theme;

    #[test]
    fn button_variant_changes_palette() {
        let theme = Theme::dark();
        let primary = Button::new("Open").primary().palette(&theme);
        let danger = Button::new("Delete").danger().palette(&theme);
        let secondary = Button::new("Cancel")
            .variant(ButtonVariant::Secondary)
            .palette(&theme);

        assert_ne!(primary.background, secondary.background);
        assert_ne!(danger.background, secondary.background);
    }
}
