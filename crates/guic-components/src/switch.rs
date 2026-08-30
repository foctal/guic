use crate::{BoolHandler, ComponentSize};
use gpui::{
    App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// A binary switch control.
#[derive(gpui::IntoElement)]
pub struct Switch {
    id: SharedString,
    label: Option<SharedString>,
    checked: bool,
    disabled: bool,
    size: ComponentSize,
    focus_handle: Option<FocusHandle>,
    on_toggle: Option<BoolHandler>,
}

impl Switch {
    /// Creates a new switch.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: None,
            checked: false,
            disabled: false,
            size: ComponentSize::Medium,
            focus_handle: None,
            on_toggle: None,
        }
    }

    /// Sets the switch label.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the switch size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Makes the switch keyboard-focusable.
    ///
    /// When focused, `Space` or `Enter` toggles the switch.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a toggle handler.
    #[must_use]
    pub fn on_toggle(mut self, on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (width, height, knob) = match self.size {
            ComponentSize::Small => (px(28.0), px(16.0), px(12.0)),
            ComponentSize::Medium => (px(36.0), px(20.0), px(16.0)),
            ComponentSize::Large => (px(44.0), px(24.0), px(20.0)),
        };
        let inset = px(2.0);
        let x = if self.checked {
            width - knob - inset * 2.0
        } else {
            px(0.0)
        };

        let switch = div()
            .relative()
            .w(width)
            .h(height)
            .rounded(height)
            .bg(if self.checked {
                theme.primary()
            } else {
                theme.secondary()
            })
            .child(
                div()
                    .absolute()
                    .top(inset)
                    .left(inset + x)
                    .size(knob)
                    .rounded_full()
                    .bg(gpui::white()),
            );

        let accessibility_label = self.label.clone().unwrap_or_else(|| self.id.clone());
        let mut row = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Switch)
                    .label(accessibility_label)
                    .checked(self.checked)
                    .disabled(self.disabled),
            )
            .flex()
            .gap_2()
            .items_center()
            .text_color(if self.disabled {
                theme.muted_foreground()
            } else {
                theme.foreground()
            })
            .child(switch);

        if let Some(label) = self.label {
            row = row.child(div().child(label));
        }

        if self.disabled {
            row.opacity(0.5).into_any_element()
        } else if let Some(on_toggle) = self.on_toggle {
            let next = !self.checked;
            let click_handler = on_toggle.clone();
            let row = row
                .cursor_pointer()
                .hover(|style: gpui::StyleRefinement| style.opacity(0.92))
                .on_click(move |_event: &ClickEvent, window, cx| {
                    (click_handler)(&next, window, cx)
                });
            if let Some(handle) = self.focus_handle {
                row.track_focus(&handle)
                    .focus_visible({
                        let ring = theme.ring();
                        move |style| style.border_color(ring)
                    })
                    .rounded(px(theme.radius.full))
                    .border_1()
                    .border_color(theme.background().opacity(0.0))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "space" | "enter") {
                            (on_toggle)(&next, window, cx);
                        }
                    })
                    .into_any_element()
            } else {
                row.into_any_element()
            }
        } else {
            row.into_any_element()
        }
    }
}
