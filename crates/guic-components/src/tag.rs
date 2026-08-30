use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    App, ClickEvent, Empty, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString,
    Styled as _, Window, div, px,
};
use guic_icons::IconName;
use guic_tokens::Theme;
use std::rc::Rc;

/// Semantic color variants for a [`Tag`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TagVariant {
    /// Neutral, low-emphasis tag.
    #[default]
    Neutral,
    /// Primary accent tag.
    Primary,
    /// Positive / success tag.
    Success,
    /// Cautionary tag.
    Warning,
    /// Destructive / error tag.
    Danger,
    /// Informational tag.
    Info,
}

/// A rectangular, optionally removable label for categorization and filtering.
///
/// Unlike [`Badge`](crate::Badge), a `Tag` uses a tinted surface, supports a
/// leading dot, and can expose a remove affordance for filter-chip workflows.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Tag, TagVariant};
///
/// Tag::new("backend")
///     .variant(TagVariant::Info)
///     .on_remove(|_, _, _| { /* drop the filter */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Tag {
    text: SharedString,
    variant: TagVariant,
    size: ComponentSize,
    dot: bool,
    removable: bool,
    on_remove: Option<ClickHandler>,
}

impl Tag {
    /// Creates a new tag with the given text.
    #[must_use]
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            variant: TagVariant::Neutral,
            size: ComponentSize::Medium,
            dot: false,
            removable: false,
            on_remove: None,
        }
    }

    /// Sets the tag variant.
    #[must_use]
    pub fn variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the tag size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Shows a leading status dot tinted to the variant.
    #[must_use]
    pub fn dot(mut self, dot: bool) -> Self {
        self.dot = dot;
        self
    }

    /// Enables a remove affordance without registering a handler.
    #[must_use]
    pub fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self
    }

    /// Registers a remove handler and enables the remove affordance.
    #[must_use]
    pub fn on_remove(
        mut self,
        on_remove: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.removable = true;
        self.on_remove = Some(Rc::new(on_remove));
        self
    }

    fn accent(&self, theme: &Theme) -> Hsla {
        match self.variant {
            TagVariant::Neutral => theme.muted_foreground(),
            TagVariant::Primary => theme.primary(),
            TagVariant::Success => theme.success(),
            TagVariant::Warning => theme.warning(),
            TagVariant::Danger => theme.danger(),
            TagVariant::Info => theme.info(),
        }
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let accent = self.accent(theme);
        let (background, foreground) = match self.variant {
            TagVariant::Neutral => (theme.secondary().opacity(0.6), theme.foreground()),
            _ => (accent.opacity(0.14), accent),
        };
        let (px_x, px_y, text_size, dot_size) = match self.size {
            ComponentSize::Small => (px(6.), px(1.), px(theme.typography.text_sm), px(6.)),
            ComponentSize::Medium => (px(8.), px(2.), px(theme.typography.text_sm), px(7.)),
            ComponentSize::Large => (px(10.), px(3.), px(theme.typography.text_md), px(8.)),
        };

        let mut root = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px(px_x)
            .py(px_y)
            .rounded(px(theme.radius.sm))
            .border_1()
            .border_color(accent.opacity(0.3))
            .bg(background)
            .text_color(foreground)
            .text_size(text_size);

        if self.dot {
            root = root.child(div().size(dot_size).rounded_full().bg(accent));
        }

        root = root.child(self.text);

        root.child(if self.removable {
            IconButton::new(IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ComponentSize::Small)
                .label("Remove tag")
                .on_click_option(self.on_remove)
                .into_any_element()
        } else {
            Empty.into_any_element()
        })
    }
}
