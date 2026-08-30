use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    App, ClickEvent, Empty, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::IconName;
use guic_tokens::Theme;
use std::rc::Rc;

/// A selectable compact value used for filters, suggestions, and choices.
///
/// Unlike [`Tag`](crate::Tag), a chip has an explicit selected state and a
/// primary activation callback. Applications own the selected state and update
/// it from [`Chip::on_click`].
#[derive(gpui::IntoElement)]
pub struct Chip {
    label: SharedString,
    selected: bool,
    disabled: bool,
    removable: bool,
    size: ComponentSize,
    on_click: Option<ClickHandler>,
    on_remove: Option<ClickHandler>,
}

impl Chip {
    /// Creates a chip with the supplied label.
    #[must_use]
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            disabled: false,
            removable: false,
            size: ComponentSize::Medium,
            on_click: None,
            on_remove: None,
        }
    }

    /// Sets whether the chip is selected.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets whether the chip is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the chip size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Enables a remove affordance without registering a handler.
    #[must_use]
    pub fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self
    }

    /// Registers the primary activation handler.
    #[must_use]
    pub fn on_click(
        mut self,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Registers a remove handler and enables the remove affordance.
    #[must_use]
    pub fn on_remove(
        mut self,
        on_remove: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.removable = true;
        self.on_remove = Some(Rc::new(on_remove));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (height, horizontal_padding, text_size) = match self.size {
            ComponentSize::Small => (px(26.0), px(8.0), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(32.0), px(10.0), px(theme.typography.text_sm)),
            ComponentSize::Large => (px(40.0), px(12.0), px(theme.typography.text_md)),
        };
        let (background, border, foreground) = if self.selected {
            (
                theme.primary().opacity(0.16),
                theme.primary().opacity(0.6),
                theme.primary(),
            )
        } else {
            (theme.background(), theme.border(), theme.foreground())
        };
        let chip = div()
            .id(format!("guic-chip-{}", self.label))
            .accessibility(
                AccessibilityProps::new(Role::Button)
                    .label(self.label.clone())
                    .selected(self.selected)
                    .disabled(self.disabled),
            )
            .h(height)
            .px(horizontal_padding)
            .rounded_full()
            .border_1()
            .border_color(border)
            .bg(background)
            .text_color(foreground)
            .text_size(text_size)
            .flex()
            .items_center()
            .gap_1p5()
            .hover({
                let hover = theme.secondary().opacity(0.34);
                move |style: gpui::StyleRefinement| style.bg(hover)
            })
            .child(self.label);

        let chip = if self.disabled {
            chip.opacity(0.5)
        } else if let Some(on_click) = self.on_click {
            let keyboard_handler = on_click.clone();
            chip.tab_index(0)
                .key_context("GuicChip")
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyboard_handler(&ClickEvent::default(), window, cx);
                        cx.stop_propagation();
                    }
                })
                .on_click(move |event, window, cx| (on_click)(event, window, cx))
        } else {
            chip
        };

        chip.child(if self.removable {
            IconButton::new(IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ComponentSize::Small)
                .label("Remove chip")
                .on_click_option(if self.disabled { None } else { self.on_remove })
                .into_any_element()
        } else {
            Empty.into_any_element()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Chip;

    #[test]
    fn chip_builder_tracks_state() {
        let chip = Chip::new("Rust")
            .selected(true)
            .disabled(true)
            .removable(true);
        assert!(chip.selected);
        assert!(chip.disabled);
        assert!(chip.removable);
    }
}
