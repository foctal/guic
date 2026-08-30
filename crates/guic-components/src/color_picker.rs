use crate::Label;
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type ColorHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// A selectable color swatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorSwatch {
    value: SharedString,
    label: SharedString,
}

impl ColorSwatch {
    /// Creates a new swatch from a CSS-style color string and label.
    #[must_use]
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    /// Returns the swatch value.
    #[must_use]
    pub fn value(&self) -> &SharedString {
        &self.value
    }
}

/// A controlled color picker with swatch selection.
#[derive(gpui::IntoElement)]
pub struct ColorPicker {
    id: SharedString,
    value: Option<SharedString>,
    swatches: Vec<ColorSwatch>,
    disabled: bool,
    on_change: Option<ColorHandler>,
}

impl ColorPicker {
    /// Creates an empty color picker.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: None,
            swatches: Vec::new(),
            disabled: false,
            on_change: None,
        }
    }

    /// Sets the selected color value.
    #[must_use]
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Replaces the available swatches.
    #[must_use]
    pub fn swatches(mut self, swatches: Vec<ColorSwatch>) -> Self {
        self.swatches = swatches;
        self
    }

    /// Sets whether the picker is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Registers a handler for requested color changes.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ColorPicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let selected = self.value.clone();
        let mut root = div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap(px(theme.spacing.x2))
            .opacity(if self.disabled { 0.55 } else { 1.0 });

        if let Some(value) = &self.value {
            root = root.child(Label::new(format!("Selected: {value}")).muted(true));
        }

        let mut swatch_row = div().flex().items_center().gap(px(theme.spacing.x2));
        let swatch_count = self.swatches.len();
        for (index, swatch) in self.swatches.into_iter().enumerate() {
            let is_selected = selected.as_ref() == Some(&swatch.value);
            let value = swatch.value.clone();
            let label = swatch.label.clone();
            let swatch_color = Rgba::try_from(value.as_ref())
                .map(gpui::Hsla::from)
                .unwrap_or_else(|_| theme.secondary());
            let contrast = if swatch_color.l < 0.5 {
                gpui::white()
            } else {
                gpui::black()
            };
            let mut swatch_view = div()
                .id(format!("guic-color-swatch-{}", value))
                .accessibility(
                    AccessibilityProps::new(Role::Radio)
                        .label(label.clone())
                        .selected(is_selected)
                        .disabled(self.disabled),
                )
                .debug_selector({
                    let value = value.clone();
                    move || format!("guic-color-swatch-{value}")
                })
                .w(px(32.0))
                .h(px(32.0))
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(if is_selected {
                    theme.primary()
                } else {
                    theme.border()
                })
                .bg(swatch_color)
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(contrast)
                        .child(
                            label
                                .chars()
                                .next()
                                .map_or(String::new(), |ch| ch.to_string()),
                        ),
                );

            if !self.disabled
                && let Some(handler) = self.on_change.clone()
            {
                let keyboard_handler = handler.clone();
                let keyboard_value = value.clone();
                swatch_view = swatch_view
                    .tab_index(0)
                    .key_context("GuicColorSwatch")
                    .cursor_pointer()
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let handled = if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            keyboard_handler(&keyboard_value, window, cx);
                            true
                        } else {
                            crate::handle_roving_focus_key(event, index, swatch_count, window, cx)
                        };
                        if handled {
                            cx.stop_propagation();
                        }
                    })
                    .on_click(move |event: &ClickEvent, window, cx| {
                        let _ = event;
                        handler(&value, window, cx);
                    });
            }
            swatch_row = swatch_row.child(swatch_view);
        }

        root.child(swatch_row)
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorPicker, ColorSwatch};

    #[test]
    fn color_picker_tracks_selected_value_and_swatches() {
        let picker = ColorPicker::new("theme-color")
            .value("#ff0000")
            .swatches(vec![ColorSwatch::new("#ff0000", "Red")])
            .disabled(true);

        assert_eq!(picker.value.as_deref(), Some("#ff0000"));
        assert_eq!(picker.swatches[0].value(), "#ff0000");
        assert!(picker.disabled);
    }
}
