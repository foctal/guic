use crate::{ButtonVariant, ComponentSize, IconButton};
use gpui::{
    App, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::IconName;
use guic_tokens::Theme;
use std::rc::Rc;

type NumberChangeHandler = Rc<dyn Fn(&f64, &mut Window, &mut App)>;

/// A numeric stepper input with decrement/increment controls.
///
/// `InputNumber` is host-managed: supply the current [`InputNumber::value`] and
/// react to [`InputNumber::on_change`], which reports the next clamped value
/// from the step controls or (when [`InputNumber::focusable`]) the up/down
/// arrow keys.
///
/// # Example
///
/// ```no_run
/// use guic_components::InputNumber;
///
/// InputNumber::new("quantity")
///     .value(3.0)
///     .range(0.0, 99.0)
///     .step(1.0)
///     .on_change(|value, _, _| { /* store */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct InputNumber {
    id: SharedString,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    disabled: bool,
    suffix: Option<SharedString>,
    size: ComponentSize,
    focus_handle: Option<FocusHandle>,
    on_change: Option<NumberChangeHandler>,
}

impl InputNumber {
    /// Creates a new numeric input over `0.0..=100.0` with step `1.0`.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            disabled: false,
            suffix: None,
            size: ComponentSize::Medium,
            focus_handle: None,
            on_change: None,
        }
    }

    /// Sets the current value.
    ///
    /// The value is clamped into the current range. Non-finite values are
    /// ignored.
    #[must_use]
    pub fn value(mut self, value: f64) -> Self {
        if value.is_finite() {
            self.value = self.clamp(value);
        }
        self
    }

    /// Sets the inclusive value range.
    ///
    /// The current value is re-clamped into the range. Non-finite bounds are
    /// ignored.
    #[must_use]
    pub fn range(mut self, min: f64, max: f64) -> Self {
        if !min.is_finite() || !max.is_finite() {
            return self;
        }
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = min;
        self.max = max;
        self.value = self.clamp(self.value);
        self
    }

    /// Sets the increment/decrement step.
    ///
    /// Non-finite steps are ignored.
    #[must_use]
    pub fn step(mut self, step: f64) -> Self {
        if step.is_finite() {
            self.step = step.abs();
        }
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets a trailing unit suffix (for example, `"px"` or `"%"`).
    #[must_use]
    pub fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Sets the control size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Makes the control keyboard-focusable so up/down arrows adjust the value.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a change handler invoked with the next clamped value.
    #[must_use]
    pub fn on_change(mut self, on_change: impl Fn(&f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// Clamps `value` into the configured range.
    fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    fn display(&self) -> String {
        let formatted = format!("{}", self.value);
        match &self.suffix {
            Some(suffix) => format!("{formatted} {suffix}"),
            None => formatted,
        }
    }
}

impl RenderOnce for InputNumber {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let accessibility_label = self.id.clone();
        let (height, text_size) = match self.size {
            ComponentSize::Small => (px(30.0), px(theme.typography.text_sm)),
            ComponentSize::Medium => (px(36.0), px(theme.typography.text_md)),
            ComponentSize::Large => (px(44.0), px(theme.typography.text_lg)),
        };

        let at_min = self.value <= self.min;
        let at_max = self.value >= self.max;
        let decremented = self.clamp(self.value - self.step);
        let incremented = self.clamp(self.value + self.step);

        let decrement = {
            let on_change = self.on_change.clone();
            IconButton::new(IconName::Minus)
                .variant(ButtonVariant::Secondary)
                .size(ComponentSize::Small)
                .label("Decrement")
                .disabled(self.disabled || at_min)
                .on_click(move |_event, window, cx| {
                    if let Some(handler) = on_change.as_ref() {
                        handler(&decremented, window, cx);
                    }
                })
        };
        let increment = {
            let on_change = self.on_change.clone();
            IconButton::new(IconName::Plus)
                .variant(ButtonVariant::Secondary)
                .size(ComponentSize::Small)
                .label("Increment")
                .disabled(self.disabled || at_max)
                .on_click(move |_event, window, cx| {
                    if let Some(handler) = on_change.as_ref() {
                        handler(&incremented, window, cx);
                    }
                })
        };

        let value_box = div()
            .flex_1()
            .h(height)
            .min_w(px(56.0))
            .px(px(theme.spacing.x3))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .text_size(text_size)
            .text_color(theme.foreground())
            .child(self.display());

        let mut root = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::SpinButton)
                    .label(accessibility_label)
                    .disabled(self.disabled)
                    .numeric_value(self.value)
                    .numeric_range(self.min, self.max),
            )
            .debug_selector({
                let selector = format!("guic-input-number-{}", self.id);
                move || selector.clone()
            })
            .flex()
            .items_center()
            .gap_2()
            .child(decrement)
            .child(value_box)
            .child(increment);

        if let Some(handle) = self.focus_handle {
            let on_change = self.on_change.clone();
            let value = self.value;
            let min = self.min;
            let max = self.max;
            let step = self.step;
            let disabled = self.disabled;
            root = root
                .key_context("GuicInputNumber")
                .track_focus(&handle)
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if disabled {
                        return;
                    }
                    let next = match event.keystroke.key.as_str() {
                        "up" => Some((value + step).clamp(min, max)),
                        "down" => Some((value - step).clamp(min, max)),
                        "home" => Some(min),
                        "end" => Some(max),
                        _ => None,
                    };
                    if let (Some(next), Some(handler)) = (next, on_change.as_ref()) {
                        handler(&next, window, cx);
                    }
                });
        }

        if self.disabled {
            root = root.opacity(0.6);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::InputNumber;

    #[test]
    fn clamp_respects_range() {
        let input = InputNumber::new("n").range(0.0, 10.0);
        assert_eq!(input.clamp(-2.0), 0.0);
        assert_eq!(input.clamp(5.0), 5.0);
        assert_eq!(input.clamp(20.0), 10.0);
    }

    #[test]
    fn display_includes_suffix() {
        let input = InputNumber::new("n").value(42.0).suffix("px");
        assert_eq!(input.display(), "42 px");
    }

    #[test]
    fn configuration_is_order_independent_and_finite() {
        let value_then_range = InputNumber::new("a").value(50.0).range(0.0, 10.0);
        let range_then_value = InputNumber::new("b").range(0.0, 10.0).value(50.0);
        assert_eq!(value_then_range.value, 10.0);
        assert_eq!(range_then_value.value, 10.0);

        let invalid = InputNumber::new("c")
            .value(5.0)
            .range(f64::NAN, 10.0)
            .value(f64::INFINITY)
            .step(f64::NAN);
        assert_eq!(invalid.value, 5.0);
        assert_eq!(invalid.min, 0.0);
        assert_eq!(invalid.max, 100.0);
        assert_eq!(invalid.step, 1.0);
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::InputNumber;
    use gpui::{
        AppContext as _, Context, FocusHandle, Keystroke, Modifiers, ParentElement as _, Render,
        Styled as _, TestAppContext, VisualContext as _, Window, div,
    };

    struct InputNumberHarness {
        value: f64,
        focus_handle: FocusHandle,
    }

    impl InputNumberHarness {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                value: 5.0,
                focus_handle: cx.focus_handle(),
            }
        }
    }

    impl Render for InputNumberHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                InputNumber::new("qty")
                    .value(self.value)
                    .range(0.0, 10.0)
                    .step(2.0)
                    .focusable(self.focus_handle.clone())
                    .on_change(cx.listener(|this, value: &f64, _, cx| {
                        this.value = *value;
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
    fn increment_and_decrement_buttons_change_value(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| InputNumberHarness::new(cx));

        let increment = cx
            .debug_bounds("guic-icon-button-Plus")
            .expect("increment button should be present");
        cx.simulate_click(increment.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.value, 7.0));

        let decrement = cx
            .debug_bounds("guic-icon-button-Minus")
            .expect("decrement button should be present");
        cx.simulate_click(decrement.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.value, 5.0));
    }

    #[gpui::test]
    fn arrow_keys_clamp_to_range(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, cx| InputNumberHarness::new(cx));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("up").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.value, 7.0));

        cx.dispatch_keystroke(window, Keystroke::parse("end").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.value, 10.0));

        // Already at max: further increments stay clamped.
        cx.dispatch_keystroke(window, Keystroke::parse("up").expect("keystroke parses"));
        view.update(cx, |view, _| assert_eq!(view.value, 10.0));
    }
}
