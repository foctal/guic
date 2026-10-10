use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    App, ClickEvent, Empty, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
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
    id: SharedString,
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
    #[track_caller]
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            id: format!("guic-chip-{}", std::panic::Location::caller()).into(),
            label: label.into(),
            selected: false,
            disabled: false,
            removable: false,
            size: ComponentSize::Medium,
            on_click: None,
            on_remove: None,
        }
    }

    /// Sets a stable logical identity, independent of the label.
    /// Required for siblings constructed from the same call site, such as a loop.
    #[must_use]
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
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
            .id(self.id.clone())
            .debug_selector({
                let selector = format!("guic-chip-{}", self.id);
                move || selector.clone()
            })
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
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(self.label);

        let chip = if self.disabled {
            chip.opacity(0.5)
        } else if let Some(on_click) = self.on_click {
            chip.tab_index(0)
                .key_context("GuicChip")
                .on_click(move |event, window, cx| (on_click)(event, window, cx))
        } else {
            chip
        };

        chip.child(if self.removable {
            IconButton::new(IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ComponentSize::Small)
                .id(format!("{}-remove", self.id))
                .label("Remove chip")
                .disabled(self.disabled)
                .on_click_option(self.on_remove.map(|handler| {
                    Rc::new(
                        move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
                            cx.stop_propagation();
                            handler(event, window, cx);
                        },
                    ) as ClickHandler
                }))
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

#[cfg(test)]
mod interaction_tests {
    use super::Chip;
    use gpui::{
        AppContext as _, Context, KeyUpEvent, Keystroke, Modifiers, ParentElement as _, Render,
        Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    struct Harness {
        activations: usize,
        removals: usize,
        disabled: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                Chip::new("Rust")
                    .id("language")
                    .disabled(self.disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.activations += 1;
                        cx.notify();
                    }))
                    .on_remove(cx.listener(|this, _, _, cx| {
                        this.removals += 1;
                        cx.notify();
                    })),
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
    fn removing_a_chip_does_not_activate_it(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            activations: 0,
            removals: 0,
            disabled: false,
        });
        let bounds = cx
            .debug_bounds("guic-icon-button-language-remove")
            .expect("remove button");
        cx.simulate_click(bounds.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(view.removals, 1);
            assert_eq!(view.activations, 0);
        });
    }

    #[gpui::test]
    fn disabled_chip_cannot_be_removed(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            activations: 0,
            removals: 0,
            disabled: true,
        });
        let bounds = cx
            .debug_bounds("guic-icon-button-language-remove")
            .expect("remove button");
        cx.simulate_click(bounds.center(), Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(view.removals, 0);
            assert_eq!(view.activations, 0);
        });
    }

    #[gpui::test]
    fn keyboard_activates_chip_once_per_press(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| Harness {
            activations: 0,
            removals: 0,
            disabled: false,
        });
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| window.focus_next(cx))
            .expect("focus");
        for (key, count) in [("enter", 1), ("space", 2)] {
            let keystroke = Keystroke::parse(key).expect("key");
            cx.dispatch_keystroke(window, keystroke.clone());
            cx.simulate_event(KeyUpEvent { keystroke });
            view.update(cx, |view, _| assert_eq!(view.activations, count));
        }
    }
}
