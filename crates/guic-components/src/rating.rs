//! A controlled star rating.
use crate::ComponentSize;
use gpui::{
    App, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

type ChangeHandler = Rc<dyn Fn(&u8, &mut Window, &mut App)>;

/// A star rating with pointer preview, keyboard navigation, and adjustable range semantics.
#[derive(gpui::IntoElement)]
pub struct Rating {
    id: SharedString,
    label: SharedString,
    value: u8,
    max: u8,
    size: ComponentSize,
    color: Option<Hsla>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Rating {
    /// Creates a five-star rating starting at zero.
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: "Rating".into(),
            value: 0,
            max: 5,
            size: ComponentSize::Medium,
            color: None,
            disabled: false,
            on_change: None,
        }
    }
    /// Sets the accessible name for this rating.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
    /// Sets the number of filled stars. Values are clamped to the maximum on display.
    #[must_use]
    pub fn value(mut self, value: u8) -> Self {
        self.value = value;
        self
    }
    /// Sets the number of available stars.
    #[must_use]
    pub fn max(mut self, max: u8) -> Self {
        self.max = max;
        self
    }
    /// Sets the size of each hit target.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }
    /// Sets the color for filled stars.
    #[must_use]
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
    /// Disables changes from all input methods.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Registers the host's selection callback.
    #[must_use]
    pub fn on_change(mut self, handler: impl Fn(&u8, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Rating {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(format!("{}-rating", self.id), cx, |_, cx| {
            (None::<u8>, cx.focus_handle().tab_stop(true))
        });
        let theme = Theme::global(cx);
        let size = self.size.control_metrics(theme).height;
        let value = self.value.min(self.max);
        let active = !self.disabled && self.max > 0 && self.on_change.is_some();
        let focus = state.read(cx).1.clone();
        let preview = if active {
            state.read(cx).0.unwrap_or(value)
        } else {
            value
        };
        let mut row = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .rounded(px(theme.radius.sm))
            .accessible(
                AccessibilityProps::new(Role::Slider)
                    .label(self.label)
                    .disabled(self.disabled)
                    .numeric_value(f64::from(value))
                    .numeric_range(0., f64::from(self.max)),
            )
            .border_1()
            .border_color(theme.background().opacity(0.))
            .focus_visible({
                let ring = theme.ring();
                move |s| s.border_color(ring)
            });
        if self.disabled {
            row = row.opacity(0.5);
        }
        let change = self.on_change.filter(|_| active).map(|handler| {
            let state = state.clone();
            Rc::new(move |score: &u8, window: &mut Window, cx: &mut App| {
                state.update(cx, |preview, cx| {
                    if preview.0.take().is_some() {
                        cx.notify();
                    }
                });
                handler(score, window, cx);
            }) as ChangeHandler
        });
        if let Some(handler) = &change {
            let max = self.max;
            let key_handler = handler.clone();
            row = row
                .track_focus(&focus)
                .aria_numeric_value_step(1.)
                .on_key_down(move |event, window, cx| {
                    let next = match event.keystroke.key.as_str() {
                        "left" | "down" => value.saturating_sub(1),
                        "right" | "up" => value.saturating_add(1).min(max),
                        "home" => 0,
                        "end" => max,
                        _ => return,
                    };
                    key_handler(&next, window, cx);
                    cx.stop_propagation();
                });
            for (action, next) in [
                (
                    gpui::accesskit::Action::Increment,
                    value.saturating_add(1).min(max),
                ),
                (gpui::accesskit::Action::Decrement, value.saturating_sub(1)),
            ] {
                let handler = handler.clone();
                row = row.on_a11y_action(action, move |_, window, cx| handler(&next, window, cx));
            }
            let handler = handler.clone();
            row = row.on_a11y_action(
                gpui::accesskit::Action::SetValue,
                move |data, window, cx| {
                    if let Some(gpui::accesskit::ActionData::NumericValue(v)) = data
                        && v.is_finite()
                    {
                        handler(&(v.round().clamp(0., f64::from(max)) as u8), window, cx);
                    }
                },
            );
        }
        for score in 1..=self.max {
            let filled = score <= preview;
            let mut star = div()
                .id(("star", usize::from(score)))
                .size(size)
                .flex()
                .items_center()
                .justify_center()
                .debug_selector({
                    let id = self.id.clone();
                    move || format!("guic-rating-{id}-{score}")
                })
                .text_size(size * 0.72)
                .text_color(if filled {
                    self.color.unwrap_or_else(|| theme.primary())
                } else {
                    theme.muted_foreground()
                })
                .child(if filled { "★" } else { "☆" });
            if let Some(handler) = change.clone() {
                let state = state.clone();
                let focus = focus.clone();
                star = star
                    .cursor_pointer()
                    .on_hover(move |hover, _, cx| {
                        state.update(cx, |s, cx| {
                            let next = if *hover {
                                Some(score)
                            } else if s.0 == Some(score) {
                                None
                            } else {
                                s.0
                            };
                            if next != s.0 {
                                s.0 = next;
                                cx.notify();
                            }
                        })
                    })
                    .on_click(move |_, window, cx| {
                        focus.focus(window, cx);
                        handler(&score, window, cx);
                    });
            }
            row = row.child(star);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CopyButton;
    use gpui::{Context, Modifiers, Render, TestAppContext};

    #[gpui::test]
    fn rating_and_copy_follow_the_controlled_value(cx: &mut TestAppContext) {
        struct Review(u8);
        impl Render for Review {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .flex()
                    .flex_col()
                    .child(Rating::new("review").value(self.0).on_change(cx.listener(
                        |s, score, _, cx| {
                            s.0 = *score;
                            cx.notify();
                        },
                    )))
                    .child(CopyButton::new("copy-review", format!("{}/5", self.0)))
            }
        }
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
        });
        let (view, cx) = cx.add_window_view(|_, _| Review(1));
        let star = cx
            .debug_bounds("guic-rating-review-4")
            .expect("fourth star");
        cx.simulate_click(star.center(), Modifiers::none());
        view.read_with(cx, |r, _| assert_eq!(r.0, 4));
        cx.simulate_keystrokes("up");
        view.read_with(cx, |r, _| assert_eq!(r.0, 5));
        cx.simulate_keystrokes("home");
        let copy = cx
            .debug_bounds("guic-button-copy-review")
            .expect("copy button");
        cx.simulate_click(copy.center(), Modifiers::none());
        cx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("0/5")
            )
        });
    }
}
