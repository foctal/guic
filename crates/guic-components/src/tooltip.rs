use gpui::{
    AnyView, App, AppContext as _, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;

/// A tooltip wrapper that uses GPUI's native tooltip deployment behavior.
#[derive(gpui::IntoElement)]
pub struct Tooltip {
    id: SharedString,
    child: gpui::AnyElement,
    message: SharedString,
}

impl Tooltip {
    /// Creates a wrapper with stable call-site identity. Use `.id(...)` in loops.
    #[must_use]
    #[track_caller]
    pub fn new(child: impl IntoElement, message: impl Into<SharedString>) -> Self {
        Self {
            id: format!("guic-tooltip-{}", std::panic::Location::caller()).into(),
            child: child.into_any_element(),
            message: message.into(),
        }
    }
    /// Sets stable identity. Use a unique ID for siblings created at the same call site.
    #[must_use]
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let message = self.message.clone();
        crate::behavioral_trigger::BehavioralTrigger::new(self.child)
            .id(self.id)
            .tooltip(move |_window, cx: &mut App| -> AnyView {
                cx.new(|_| TooltipBubble {
                    message: message.clone(),
                })
                .into()
            })
    }
}

struct TooltipBubble {
    message: SharedString,
}

impl Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::global(cx);
        div()
            .id("guic-tooltip-bubble")
            .accessibility(AccessibilityProps::new(Role::Tooltip).label(self.message.clone()))
            .px_3()
            .py_2()
            .rounded(px(theme.radius.sm))
            .bg(theme.foreground())
            .text_color(theme.background())
            .text_size(px(theme.typography.text_sm))
            .child(self.message.clone())
    }
}
