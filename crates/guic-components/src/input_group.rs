use crate::ComponentSize;
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;

/// A labelled horizontal group for an input and adjacent units or actions.
///
/// Child controls retain their own borders, accessible labels and keyboard
/// behavior. The group does not add a tab stop or intercept input events.
///
/// ```no_run
/// use guic_components::{Button, InputGroup, InputGroupAddon};
/// InputGroup::new("amount-group")
///     .label("Amount")
///     .child(InputGroupAddon::new("USD"))
///     .child(Button::new("Apply"));
/// ```
#[derive(gpui::IntoElement)]
pub struct InputGroup {
    id: SharedString,
    label: Option<SharedString>,
    children: Vec<AnyElement>,
}

impl InputGroup {
    /// Creates an empty group with a stable identity.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: None,
            children: Vec::new(),
        }
    }

    /// Sets the accessible group label. Inputs still need their own labels.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Appends an input, addon, or action. Wrap flexible inputs in a container
    /// with `flex_1().min_w_0()` when they should consume the remaining width.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for InputGroup {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut accessibility = AccessibilityProps::new(Role::Group);
        if let Some(label) = self.label {
            accessibility = accessibility.label(label);
        }
        div()
            .id(self.id)
            .accessibility(accessibility)
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(Theme::global(cx).spacing.x1))
            .children(self.children)
    }
}

/// Non-interactive text such as a currency prefix or unit suffix in an input group.
#[derive(gpui::IntoElement)]
pub struct InputGroupAddon {
    text: SharedString,
    size: ComponentSize,
}

impl InputGroupAddon {
    /// Creates an addon using the default control height.
    #[must_use]
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            size: ComponentSize::Medium,
        }
    }

    /// Matches the height and typography of sibling controls.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for InputGroupAddon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let metrics = self.size.control_metrics(theme);
        div()
            .h(metrics.height)
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(theme.spacing.x3))
            .rounded(px(theme.radius.md))
            .bg(theme.secondary())
            .text_color(theme.muted_foreground())
            .text_size(metrics.font_size)
            .child(self.text)
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::{InputGroup, InputGroupAddon};
    use crate::{Button, Skeleton, TextInput};
    use gpui::{
        AppContext as _, Context, Entity, InteractiveElement as _, KeyUpEvent, Keystroke,
        ParentElement as _, Render, Styled as _, TestAppContext, VisualContext as _, Window, div,
        px,
    };

    struct Harness {
        input: Entity<TextInput>,
        clicks: usize,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
            div()
                .w(px(240.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("placeholder")
                        .debug_selector(|| "placeholder".into())
                        .child(Skeleton::new().height(20.0)),
                )
                .child(
                    InputGroup::new("search")
                        .label("Search")
                        .child(InputGroupAddon::new("Find"))
                        .child(
                            div()
                                .id("input-slot")
                                .debug_selector(|| "input-slot".into())
                                .flex_1()
                                .min_w_0()
                                .child(self.input.clone()),
                        )
                        .child(Button::new("Go").on_click(cx.listener(|this, _, _, cx| {
                            this.clicks += 1;
                            cx.notify();
                        }))),
                )
        }
    }

    #[gpui::test]
    fn grouped_input_keeps_keyboard_order_and_fits_a_narrow_container(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let view = cx.new(|cx| Harness {
            input: cx.new(|cx| TextInput::new("query", cx)),
            clicks: 0,
        });
        let (_, cx) =
            cx.add_window_view(|window, cx| guic_core::Root::new(view.clone(), window, cx));
        let placeholder = cx.debug_bounds("placeholder").expect("placeholder bounds");
        assert_eq!(placeholder.size.width, px(240.0));
        assert_eq!(placeholder.size.height, px(20.0));
        let input = cx.debug_bounds("input-slot").expect("input bounds");
        let button = cx.debug_bounds("guic-button-Go").expect("button bounds");
        assert!(input.size.width > px(0.0));
        assert!(input.right() <= button.left());
        assert!(button.right() <= px(240.0));
        let window = cx.window_handle();
        cx.update_window(window, |_, window, cx| window.focus_next(cx))
            .expect("focus input");
        cx.dispatch_keystroke(window, Keystroke::parse("a").expect("key"));
        view.update(cx, |view, cx| {
            assert_eq!(view.input.read(cx).current_value(), "a")
        });
        cx.dispatch_keystroke(window, Keystroke::parse("tab").expect("tab key"));
        let keystroke = Keystroke::parse("enter").expect("key");
        cx.dispatch_keystroke(window, keystroke.clone());
        cx.simulate_event(KeyUpEvent { keystroke });
        view.update(cx, |view, _| assert_eq!(view.clicks, 1));
        cx.dispatch_keystroke(window, Keystroke::parse("shift-tab").expect("reverse tab"));
        cx.dispatch_keystroke(window, Keystroke::parse("b").expect("key"));
        view.update(cx, |view, cx| {
            assert_eq!(view.input.read(cx).current_value(), "ab")
        });
    }
}
