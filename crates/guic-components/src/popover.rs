use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
};
use guic_core::{OverlayPriority, overlay_portal};
use guic_tokens::Theme;

/// A controlled popover surface anchored inline below its trigger.
#[derive(gpui::IntoElement)]
pub struct Popover {
    id: SharedString,
    trigger: AnyElement,
    content: AnyElement,
    open: bool,
    width: Option<f32>,
}

impl Popover {
    /// Creates a new popover from a trigger and content element.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        trigger: impl IntoElement,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            content: content.into_any_element(),
            open: false,
            width: None,
        }
    }

    /// Sets whether the popover is open.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Sets an explicit width in pixels.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.width = Some(width.max(1.0));
        }
        self
    }
}

impl RenderOnce for Popover {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let panel_id = format!("{}-panel", self.id);
        let mut root = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_col()
            .child(self.trigger);

        if self.open {
            let mut panel = div()
                .id(panel_id)
                .absolute()
                .top_full()
                .left_0()
                .mt_2()
                .debug_selector(|| "guic-popover-panel".to_owned())
                .rounded(px(theme.radius.md))
                .border_1()
                .border_color(theme.border())
                .bg(theme.background())
                .shadow_lg()
                .p_4()
                .child(self.content);

            if let Some(width) = self.width {
                panel = panel.w(px(width)).max_w_full();
            }

            root = root.child(overlay_portal(panel, OverlayPriority::FLOATING));
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::Popover;
    use gpui::div;

    #[test]
    fn width_rejects_invalid_layout_values() {
        assert_eq!(
            Popover::new("popover", div(), div()).width(-2.0).width,
            Some(1.0)
        );
        assert_eq!(
            Popover::new("popover", div(), div())
                .width(f32::INFINITY)
                .width,
            None
        );
    }
}
