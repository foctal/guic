use gpui::{
    App, Bounds, Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Render, SharedString,
    Styled as _, Window, canvas, div, px, relative,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type ChangeHandler = Rc<dyn Fn(&f32, &mut Window, &mut App)>;

/// A draggable, keyboard-operable value slider.
///
/// `Slider` is a stateful entity (like [`TextInput`](crate::TextInput)): create
/// it once with [`Slider::new`] and store the [`gpui::Entity`]. It owns its
/// value, supports pointer drag and keyboard adjustment, and reports changes
/// through [`Slider::on_change`].
///
/// # Example
///
/// ```no_run
/// use gpui::Context;
/// use guic_components::Slider;
///
/// fn build_slider(cx: &mut Context<Slider>) -> Slider {
///     Slider::new("volume", cx)
///         .range(0.0, 100.0)
///         .step(5.0)
///         .value(40.0)
///         .on_change(|value, _, _| { /* react */ })
/// }
/// ```
pub struct Slider {
    id: SharedString,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    focus_handle: FocusHandle,
    last_track_bounds: Option<Bounds<Pixels>>,
    on_change: Option<ChangeHandler>,
}

impl Slider {
    /// Creates a new slider over the range `0.0..=100.0`.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            disabled: false,
            focus_handle: cx.focus_handle(),
            last_track_bounds: None,
            on_change: None,
        }
    }

    /// Sets the value range. The current value is re-clamped into it.
    ///
    /// Non-finite bounds are ignored.
    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if !min.is_finite() || !max.is_finite() {
            return self;
        }
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(min, max);
        self
    }

    /// Sets the discrete step. A step of `0.0` allows continuous values.
    ///
    /// Non-finite steps are ignored.
    #[must_use]
    pub fn step(mut self, step: f32) -> Self {
        if step.is_finite() {
            self.step = step.max(0.0);
        }
        self
    }

    /// Sets the initial value (clamped into the current range).
    ///
    /// Non-finite values are ignored.
    #[must_use]
    pub fn value(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.value = value.clamp(self.min, self.max);
        }
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Registers a change handler invoked with the new value.
    #[must_use]
    pub fn on_change(mut self, on_change: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// Returns the current value.
    #[must_use]
    pub fn current_value(&self) -> f32 {
        self.value
    }

    /// Returns the value as a `0.0..=1.0` fraction of the range.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if span <= 0.0 {
            0.0
        } else {
            ((self.value - self.min) / span).clamp(0.0, 1.0)
        }
    }

    /// The effective step used for keyboard adjustment.
    fn keyboard_step(&self) -> f32 {
        if self.step > 0.0 {
            self.step
        } else {
            (self.max - self.min) / 100.0
        }
    }

    fn snap(&self, raw: f32) -> f32 {
        let clamped = raw.clamp(self.min, self.max);
        if self.step > 0.0 {
            let steps = ((clamped - self.min) / self.step).round();
            (self.min + steps * self.step).clamp(self.min, self.max)
        } else {
            clamped
        }
    }

    fn value_from_x(&self, x: Pixels) -> f32 {
        let Some(bounds) = self.last_track_bounds else {
            return self.value;
        };
        let width = f32::from(bounds.size.width);
        if width <= 0.0 {
            return self.value;
        }
        let fraction = ((f32::from(x) - f32::from(bounds.origin.x)) / width).clamp(0.0, 1.0);
        self.min + fraction * (self.max - self.min)
    }

    fn apply_value(&mut self, raw: f32, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.snap(raw);
        if (next - self.value).abs() > f32::EPSILON {
            self.value = next;
            if let Some(on_change) = self.on_change.clone() {
                on_change(&next, window, cx);
            }
            cx.notify();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let step = self.keyboard_step();
        let next = match event.keystroke.key.as_str() {
            "left" | "down" => Some(self.value - step),
            "right" | "up" => Some(self.value + step),
            "home" => Some(self.min),
            "end" => Some(self.max),
            _ => None,
        };
        if let Some(next) = next {
            self.apply_value(next, window, cx);
        }
    }
}

impl Render for Slider {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        let fraction = self.fraction();
        let disabled = self.disabled;
        let selector = format!("guic-slider-{}", self.id);

        let bounds_sink = cx.entity();
        let track_bounds_canvas = canvas(
            move |bounds, _window, app| {
                bounds_sink.update(app, |slider, _| {
                    slider.last_track_bounds = Some(bounds);
                });
            },
            |_bounds, _state, _window, _app| {},
        )
        .absolute()
        .inset_0();

        let fill = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(relative(fraction))
            .rounded_full()
            .bg(if disabled {
                theme.muted_foreground()
            } else {
                theme.primary()
            });

        let thumb = div()
            .absolute()
            .top(px(-5.))
            .left(relative(fraction))
            .ml(px(-8.))
            .size(px(16.))
            .rounded_full()
            .border_2()
            .border_color(theme.background())
            .bg(if disabled {
                theme.muted_foreground()
            } else {
                theme.primary()
            })
            .shadow_sm();

        let track = div()
            .relative()
            .w_full()
            .h(px(6.))
            .rounded_full()
            .bg(theme.secondary())
            .child(fill)
            .child(thumb)
            .child(track_bounds_canvas);

        let mut root = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Slider)
                    .label(self.id.clone())
                    .disabled(disabled)
                    .numeric_value(self.value.into())
                    .numeric_range(self.min.into(), self.max.into()),
            )
            .key_context("GuicSlider")
            .track_focus(&self.focus_handle)
            .debug_selector(move || selector.clone())
            .w_full()
            .py(px(8.))
            .rounded(px(theme.radius.sm))
            .border_1()
            .border_color(theme.background().opacity(0.0))
            .focus_visible({
                let ring = theme.ring();
                move |style| style.border_color(ring)
            })
            .child(track)
            .on_key_down(cx.listener(Self::on_key_down));

        if disabled {
            root = root.opacity(0.55);
        } else {
            root = root
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        let value = this.value_from_x(event.position.x);
                        this.apply_value(value, window, cx);
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        let value = this.value_from_x(event.position.x);
                        this.apply_value(value, window, cx);
                    }
                }));
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::Slider;
    use gpui::{AppContext as _, Context, TestAppContext};

    fn with_slider(cx: &mut TestAppContext, f: impl FnOnce(&mut Slider, &mut Context<Slider>)) {
        let slider = cx.new(|cx| Slider::new("test", cx).range(0.0, 100.0).step(10.0));
        slider.update(cx, |slider, cx| f(slider, cx));
    }

    #[gpui::test]
    fn snap_rounds_to_step(cx: &mut TestAppContext) {
        with_slider(cx, |slider, _| {
            assert_eq!(slider.snap(43.0), 40.0);
            assert_eq!(slider.snap(46.0), 50.0);
            assert_eq!(slider.snap(-5.0), 0.0);
            assert_eq!(slider.snap(140.0), 100.0);
        });
    }

    #[gpui::test]
    fn fraction_reflects_value(cx: &mut TestAppContext) {
        with_slider(cx, |slider, _| {
            slider.value = 25.0;
            assert!((slider.fraction() - 0.25).abs() < f32::EPSILON);
        });
    }

    #[gpui::test]
    fn keyboard_step_falls_back_when_continuous(cx: &mut TestAppContext) {
        let slider = cx.new(|cx| Slider::new("c", cx).range(0.0, 50.0).step(0.0));
        slider.update(cx, |slider, _| {
            assert!((slider.keyboard_step() - 0.5).abs() < f32::EPSILON);
        });
    }

    #[gpui::test]
    fn non_finite_configuration_is_ignored(cx: &mut TestAppContext) {
        let slider = cx.new(|cx| {
            Slider::new("safe", cx)
                .range(10.0, 20.0)
                .value(15.0)
                .range(f32::NAN, 100.0)
                .step(f32::INFINITY)
                .value(f32::NAN)
        });
        slider.update(cx, |slider, _| {
            assert_eq!(slider.min, 10.0);
            assert_eq!(slider.max, 20.0);
            assert_eq!(slider.current_value(), 15.0);
            assert_eq!(slider.step, 1.0);
            assert!(slider.fraction().is_finite());
        });
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::Slider;
    use gpui::{
        AppContext as _, Keystroke, Modifiers, MouseButton, TestAppContext, VisualContext as _,
        point,
    };

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn click_sets_value_from_track_position(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) =
            cx.add_window_view(|_, cx| Slider::new("s", cx).range(0.0, 100.0).step(10.0));

        let bounds = cx
            .debug_bounds("guic-slider-s")
            .expect("slider bounds should be present");
        cx.simulate_click(bounds.center(), Modifiers::none());

        view.update(cx, |slider, _| {
            assert_eq!(slider.current_value(), 50.0);
        });
    }

    #[gpui::test]
    fn drag_updates_value_continuously(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) =
            cx.add_window_view(|_, cx| Slider::new("s", cx).range(0.0, 100.0).step(10.0));

        let bounds = cx
            .debug_bounds("guic-slider-s")
            .expect("slider bounds should be present");
        let left = bounds.origin.x;
        let width = bounds.size.width;
        let y = bounds.center().y;
        let at = |fraction: f32| point(left + width * fraction, y);

        cx.simulate_mouse_down(at(0.2), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at(0.8), MouseButton::Left, Modifiers::none());

        view.update(cx, |slider, _| {
            assert_eq!(slider.current_value(), 80.0);
        });
    }

    #[gpui::test]
    fn arrow_keys_adjust_value(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) =
            cx.add_window_view(|_, cx| Slider::new("s", cx).range(0.0, 100.0).step(10.0));
        let window = cx.window_handle();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |slider, cx| slider.focus_handle.focus(window, cx));
        })
        .expect("window update should succeed");

        cx.dispatch_keystroke(window, Keystroke::parse("right").expect("keystroke parses"));
        cx.dispatch_keystroke(window, Keystroke::parse("right").expect("keystroke parses"));
        view.update(cx, |slider, _| assert_eq!(slider.current_value(), 20.0));

        cx.dispatch_keystroke(window, Keystroke::parse("end").expect("keystroke parses"));
        view.update(cx, |slider, _| assert_eq!(slider.current_value(), 100.0));

        cx.dispatch_keystroke(window, Keystroke::parse("home").expect("keystroke parses"));
        view.update(cx, |slider, _| assert_eq!(slider.current_value(), 0.0));
    }
}
