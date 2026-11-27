use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, px, relative,
};
use guic_tokens::Theme;
use std::{cell::Cell, rc::Rc};

type ResizeHandler = Rc<dyn Fn(&f32, &mut Window, &mut App)>;

/// The orientation of a [`Splitter`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SplitterAxis {
    /// Places panes next to each other.
    #[default]
    Horizontal,
    /// Places panes above and below each other.
    Vertical,
}

/// A controlled two-pane layout with a visible divider.
#[derive(gpui::IntoElement)]
pub struct Splitter {
    id: SharedString,
    axis: SplitterAxis,
    fraction: f32,
    on_resize: Option<ResizeHandler>,
    first: AnyElement,
    second: AnyElement,
}

impl Splitter {
    /// Creates a horizontal splitter with an equal initial division.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        first: impl IntoElement,
        second: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            axis: SplitterAxis::Horizontal,
            fraction: 0.5,
            on_resize: None,
            first: first.into_any_element(),
            second: second.into_any_element(),
        }
    }

    /// Enables pointer, keyboard, and accessibility resizing. Store the requested
    /// fraction and supply it on the next render. Arrow keys adjust by five percent.
    #[must_use]
    pub fn on_resize(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Sets the layout orientation.
    #[must_use]
    pub fn axis(mut self, axis: SplitterAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Sets the first pane fraction, clamped to keep both panes usable.
    ///
    /// Non-finite fractions are ignored.
    #[must_use]
    pub fn fraction(mut self, fraction: f32) -> Self {
        if fraction.is_finite() {
            self.fraction = fraction.clamp(0.1, 0.9);
        }
        self
    }
}

impl RenderOnce for Splitter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let focus_color = theme.primary().opacity(0.25);
        let divider = match self.axis {
            SplitterAxis::Horizontal => div().w(px(1.)).h_full(),
            SplitterAxis::Vertical => div().h(px(1.)).w_full(),
        }
        .bg(theme.border());
        let first = match self.axis {
            SplitterAxis::Horizontal => div().w(relative(self.fraction)).h_full(),
            SplitterAxis::Vertical => div().h(relative(self.fraction)).w_full(),
        }
        .overflow_hidden()
        .child(self.first);
        let second = match self.axis {
            SplitterAxis::Horizontal => div().flex_1().h_full(),
            SplitterAxis::Vertical => div().flex_1().w_full(),
        }
        .overflow_hidden()
        .child(self.second);
        let mut root = div().id(self.id.clone()).relative().size_full().flex();
        let divider = if let Some(handler) = self.on_resize {
            let bounds = Rc::new(Cell::new(None::<gpui::Bounds<gpui::Pixels>>));
            let sink = bounds.clone();
            root = root.child(
                canvas(move |area, _, _| sink.set(Some(area)), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            );
            let mut handle = div()
                .id("divider")
                .debug_selector(move || format!("guic-splitter-{}", self.id))
                .tab_index(0)
                .role(gpui::Role::Splitter)
                .aria_numeric_value(f64::from(self.fraction))
                .aria_min_numeric_value(0.1)
                .aria_max_numeric_value(0.9)
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .focus(move |style| style.bg(focus_color));
            let axis = self.axis;
            handle = match axis {
                SplitterAxis::Horizontal => handle
                    .w(px(8.0))
                    .h_full()
                    .cursor_col_resize()
                    .aria_orientation(gpui::accesskit::Orientation::Vertical),
                SplitterAxis::Vertical => handle
                    .h(px(8.0))
                    .w_full()
                    .cursor_row_resize()
                    .aria_orientation(gpui::accesskit::Orientation::Horizontal),
            };
            let fraction = self.fraction;
            for (action, step) in [
                (gpui::accesskit::Action::Increment, 0.05),
                (gpui::accesskit::Action::Decrement, -0.05),
            ] {
                let handler = handler.clone();
                handle = handle.on_a11y_action(action, move |_, window, cx| {
                    handler(&(fraction + step).clamp(0.1, 0.9), window, cx)
                });
            }
            handle
                .on_key_down({
                    let handler = handler.clone();
                    move |event, window, cx| {
                        let next = match (axis, event.keystroke.key.as_str()) {
                            (SplitterAxis::Horizontal, "left") | (SplitterAxis::Vertical, "up") => {
                                fraction - 0.05
                            }
                            (SplitterAxis::Horizontal, "right")
                            | (SplitterAxis::Vertical, "down") => fraction + 0.05,
                            (_, "home") => 0.1,
                            (_, "end") => 0.9,
                            _ => return,
                        };
                        handler(&next.clamp(0.1, 0.9), window, cx);
                        cx.stop_propagation();
                    }
                })
                .on_drag_update(move |_, event, window, cx| {
                    let Some(area) = bounds.get() else {
                        return;
                    };
                    let (offset, extent) = match axis {
                        SplitterAxis::Horizontal => {
                            (event.position.x - area.left(), area.size.width)
                        }
                        SplitterAxis::Vertical => (event.position.y - area.top(), area.size.height),
                    };
                    if extent > px(0.) {
                        handler(&((offset - px(4.0)) / extent).clamp(0.1, 0.9), window, cx);
                    }
                })
                .child(divider)
                .into_any_element()
        } else {
            divider.into_any_element()
        };
        match self.axis {
            SplitterAxis::Horizontal => root.flex_row().child(first).child(divider).child(second),
            SplitterAxis::Vertical => root.flex_col().child(first).child(divider).child(second),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Splitter;
    use gpui::div;

    #[gpui::test]
    fn dragging_and_keyboard_resizing_update_the_controlled_fraction(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::{
            Context, IntoElement, Modifiers, MouseButton, ParentElement as _, Render, Styled as _,
            Window, point, px,
        };
        struct Panes(f32);
        impl Render for Panes {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().w(px(400.)).h(px(200.)).child(
                    Splitter::new("test", div(), div())
                        .fraction(self.0)
                        .on_resize(cx.listener(|panes, value, _, cx| {
                            panes.0 = *value;
                            cx.notify();
                        })),
                )
            }
        }
        cx.update(guic_tokens::init);
        let (view, cx) = cx.add_window_view(|_, _| Panes(0.5));
        let center = cx
            .debug_bounds("guic-splitter-test")
            .expect("divider")
            .center();
        let end = point(px(304.), center.y);
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
        view.read_with(cx, |panes, _| assert_eq!(panes.0, 0.75));
        cx.simulate_keystrokes("home right");
        view.read_with(cx, |panes, _| assert!((panes.0 - 0.15).abs() < 0.0001));
    }

    #[test]
    fn clamps_pane_fraction() {
        assert_eq!(
            Splitter::new("split", div(), div()).fraction(2.).fraction,
            0.9
        );
        assert_eq!(
            Splitter::new("split", div(), div())
                .fraction(f32::NAN)
                .fraction,
            0.5
        );
    }
}
