use crate::{BoolHandler, ComponentSize};
use gpui::{
    App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// A standalone radio control.
#[derive(gpui::IntoElement)]
pub struct Radio {
    id: SharedString,
    label: Option<SharedString>,
    checked: bool,
    disabled: bool,
    size: ComponentSize,
    focus_handle: Option<FocusHandle>,
    on_select: Option<BoolHandler>,
}

impl Radio {
    /// Creates a new radio control.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: None,
            checked: false,
            disabled: false,
            size: ComponentSize::Medium,
            focus_handle: None,
            on_select: None,
        }
    }

    /// Sets the radio label.
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

    /// Sets the control size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Makes the radio keyboard-focusable.
    ///
    /// When focused, `Space` or `Enter` selects this radio.
    #[must_use]
    pub fn focusable(mut self, focus_handle: FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle);
        self
    }

    /// Registers a selection handler.
    #[must_use]
    pub fn on_select(mut self, on_select: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let dimension = match self.size {
            ComponentSize::Small => px(16.0),
            ComponentSize::Medium => px(18.0),
            ComponentSize::Large => px(20.0),
        };

        let outer = div()
            .size(dimension)
            .rounded_full()
            .border_1()
            .border_color(if self.checked {
                theme.primary()
            } else {
                theme.border()
            })
            .bg(theme.background())
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(match self.size {
                        ComponentSize::Small => 7.0,
                        ComponentSize::Medium => 8.0,
                        ComponentSize::Large => 10.0,
                    }))
                    .rounded_full()
                    .bg(if self.checked {
                        theme.primary()
                    } else {
                        theme.background().opacity(0.0)
                    }),
            );

        let accessibility_label = self.label.clone().unwrap_or_else(|| self.id.clone());
        let mut row = div()
            .id(self.id.clone())
            .accessibility(
                AccessibilityProps::new(Role::Radio)
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
            .child(outer);

        if let Some(label) = self.label {
            row = row.child(div().child(label));
        }

        if self.disabled {
            row.opacity(0.5).into_any_element()
        } else if let Some(on_select) = self.on_select {
            let click_handler = on_select.clone();
            let row = row
                .cursor_pointer()
                .hover(|style: gpui::StyleRefinement| style.opacity(0.92))
                .on_click(move |_event: &ClickEvent, window, cx| {
                    (click_handler)(&true, window, cx)
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
                            (on_select)(&true, window, cx);
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
