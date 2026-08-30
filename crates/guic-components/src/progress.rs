use crate::ComponentSize;
use gpui::{
    Animation, AnimationExt as _, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Styled as _, Window, div, px, relative,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::time::Duration;

/// A horizontal progress indicator.
#[derive(gpui::IntoElement)]
pub struct Progress {
    id: gpui::SharedString,
    value: f32,
    size: ComponentSize,
    indeterminate: bool,
}

impl Progress {
    /// Creates a new progress bar with a determinate value.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            id: "guic-progress".into(),
            value: if value.is_finite() {
                value.clamp(0.0, 100.0)
            } else {
                0.0
            },
            size: ComponentSize::Medium,
            indeterminate: false,
        }
    }

    /// Sets a stable element identifier.
    ///
    /// Set a distinct identifier when rendering multiple progress indicators
    /// in the same view.
    #[must_use]
    pub fn id(mut self, id: impl Into<gpui::SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// Enables indeterminate loading mode.
    #[must_use]
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Sets the component size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Returns the clamped progress value.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let height = match self.size {
            ComponentSize::Small => px(4.0),
            ComponentSize::Medium => px(8.0),
            ComponentSize::Large => px(12.0),
        };
        let accessibility = if self.indeterminate {
            AccessibilityProps::new(Role::ProgressIndicator).label("Loading")
        } else {
            AccessibilityProps::new(Role::ProgressIndicator)
                .label(format!("Progress: {}%", self.value))
                .numeric_value(self.value.into())
                .numeric_range(0.0, 100.0)
        };

        let fill = if self.indeterminate {
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(relative(0.4))
                .rounded(px(theme.radius.full))
                .bg(theme.primary())
                .with_animation(
                    "progress-indeterminate",
                    Animation::new(Duration::from_millis(theme.motion.normal_ms.into())).repeat(),
                    |this, delta| this.opacity(0.35 + (0.65 * delta)),
                )
                .into_any_element()
        } else {
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(relative((self.value / 100.0).clamp(0.0, 1.0)))
                .rounded(px(theme.radius.full))
                .bg(theme.primary())
                .into_any_element()
        };

        div()
            .id(self.id)
            .accessibility(accessibility)
            .w_full()
            .relative()
            .overflow_hidden()
            .h(height)
            .rounded(px(theme.radius.full))
            .bg(theme.secondary().opacity(0.45))
            .child(fill)
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;

    #[test]
    fn clamps_progress_value() {
        assert_eq!(Progress::new(150.0).value(), 100.0);
        assert_eq!(Progress::new(-20.0).value(), 0.0);
        assert_eq!(Progress::new(f32::NAN).value(), 0.0);
        assert_eq!(Progress::new(f32::INFINITY).value(), 0.0);
    }
}
