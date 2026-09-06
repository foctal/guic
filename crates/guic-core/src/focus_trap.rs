use gpui::{
    App, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, Styled as _, Window, div,
};

struct Handles {
    scope: FocusHandle,
    start: FocusHandle,
    end: FocusHandle,
}

/// A full-size modal focus boundary with cyclic Tab and Shift-Tab navigation.
/// Place this inside the modal portal. Nested boundaries handle navigation first.
/// Child controls must participate in GPUI's tab order.
#[derive(gpui::IntoElement)]
pub struct FocusTrap {
    id: SharedString,
    child: gpui::AnyElement,
}

impl FocusTrap {
    /// Wraps a modal surface using stable identity for its focus boundary.
    pub fn new(id: impl Into<SharedString>, child: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
        }
    }
}

impl RenderOnce for FocusTrap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| Handles {
            scope: cx.focus_handle().tab_stop(false),
            start: cx.focus_handle().tab_index(isize::MIN).tab_stop(false),
            end: cx.focus_handle().tab_index(isize::MAX).tab_stop(false),
        });
        let handles = state.read(cx);
        let scope = handles.scope.clone();
        let start = handles.start.clone();
        let end = handles.end.clone();
        div()
            .id(self.id)
            .size_full()
            .tab_group()
            .key_context("GuicModal")
            .track_focus(&scope)
            .child(
                div()
                    .absolute()
                    .tab_index(isize::MIN)
                    .tab_stop(false)
                    .track_focus(&start),
            )
            .child(self.child)
            .child(
                div()
                    .absolute()
                    .tab_index(isize::MAX)
                    .tab_stop(false)
                    .track_focus(&end),
            )
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key != "tab"
                    || modifiers.control
                    || modifiers.alt
                    || modifiers.platform
                {
                    return;
                }
                if modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                if !scope.contains_focused(window, cx) {
                    if modifiers.shift {
                        end.focus(window, cx);
                        window.focus_prev(cx);
                    } else {
                        start.focus(window, cx);
                        window.focus_next(cx);
                    }
                    if !scope.contains_focused(window, cx) {
                        scope.focus(window, cx);
                    }
                }
                cx.stop_propagation();
                window.prevent_default();
            })
    }
}
