use gpui::{App, IntoElement, RenderOnce, Styled as _, Window, div, px};
use guic_tokens::Theme;

/// A static, decorative loading placeholder that reserves space for content.
///
/// Skeletons do not schedule animation frames or create focus/accessibility
/// nodes. Announce loading once on the containing surface when appropriate.
///
/// ```no_run
/// use guic_components::Skeleton;
/// let avatar = Skeleton::new().width(40.0).height(40.0).circular(true);
/// let text = Skeleton::new().height(16.0);
/// ```
#[derive(gpui::IntoElement)]
pub struct Skeleton {
    width: Option<f32>,
    height: f32,
    circular: bool,
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl Skeleton {
    /// Creates a full-width placeholder with a height of 16 logical pixels.
    #[must_use]
    pub fn new() -> Self {
        Self {
            width: None,
            height: 16.0,
            circular: false,
        }
    }

    /// Sets the width in logical pixels. Negative values become zero;
    /// non-finite values leave the existing width unchanged.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.width = Some(width.max(0.0));
        }
        self
    }

    /// Sets the height in logical pixels. Negative values become zero;
    /// non-finite values leave the existing height unchanged.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.height = height.max(0.0);
        }
        self
    }

    /// Uses fully rounded corners. Set equal width and height for a circle.
    #[must_use]
    pub fn circular(mut self, circular: bool) -> Self {
        self.circular = circular;
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .h(px(self.height))
            .flex_shrink_0()
            .bg(theme.secondary());
        root = match self.width {
            Some(width) => root.w(px(width)),
            None => root.w_full(),
        };
        if self.circular {
            root.rounded_full()
        } else {
            root.rounded(px(theme.radius.md))
        }
    }
}
