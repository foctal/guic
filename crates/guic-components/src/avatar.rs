use crate::ComponentSize;
use gpui::{
    App, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div,
    px,
};
use guic_tokens::Theme;

/// The visual shape of an [`Avatar`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AvatarShape {
    /// Fully rounded circular avatar.
    #[default]
    Circle,
    /// Rounded-rectangle avatar.
    Rounded,
}

/// A presence indicator rendered in the corner of an [`Avatar`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AvatarStatus {
    /// Active / online presence.
    Online,
    /// Idle / away presence.
    Away,
    /// Do-not-disturb / busy presence.
    Busy,
    /// Offline presence.
    Offline,
}

/// A compact identity surface that renders initials with an optional status dot.
///
/// `Avatar` is a presentational widget: it derives initials from a name and
/// applies a deterministic accent color so the same identity always renders the
/// same way.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Avatar, AvatarStatus};
///
/// Avatar::new("Ada Lovelace").status(AvatarStatus::Online);
/// ```
#[derive(gpui::IntoElement)]
pub struct Avatar {
    label: SharedString,
    initials: Option<SharedString>,
    shape: AvatarShape,
    size: ComponentSize,
    status: Option<AvatarStatus>,
}

impl Avatar {
    /// Creates a new avatar for the given display name.
    #[must_use]
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            initials: None,
            shape: AvatarShape::Circle,
            size: ComponentSize::Medium,
            status: None,
        }
    }

    /// Overrides the auto-derived initials.
    #[must_use]
    pub fn initials(mut self, initials: impl Into<SharedString>) -> Self {
        self.initials = Some(initials.into());
        self
    }

    /// Sets the avatar shape.
    #[must_use]
    pub fn shape(mut self, shape: AvatarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Sets the avatar size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets a presence indicator.
    #[must_use]
    pub fn status(mut self, status: AvatarStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Derives up to two uppercase initials from the label.
    fn derived_initials(&self) -> SharedString {
        if let Some(initials) = &self.initials {
            return initials.clone();
        }

        let mut chars: Vec<char> = self
            .label
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        chars.truncate(2);
        if chars.is_empty() {
            SharedString::from("?")
        } else {
            SharedString::from(chars.into_iter().collect::<String>())
        }
    }

    /// Picks a deterministic accent hue from the label so identities are stable.
    fn accent(&self) -> Hsla {
        let seed = self.label.bytes().fold(0u32, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(u32::from(b))
        });
        let hue = (seed % 360) as f32 / 360.0;
        Hsla {
            h: hue,
            s: 0.55,
            l: 0.45,
            a: 1.0,
        }
    }
}

fn status_color(status: AvatarStatus, theme: &Theme) -> Hsla {
    match status {
        AvatarStatus::Online => theme.success(),
        AvatarStatus::Away => theme.warning(),
        AvatarStatus::Busy => theme.danger(),
        AvatarStatus::Offline => theme.muted_foreground(),
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let (dimension, text_size, dot) = match self.size {
            ComponentSize::Small => (px(24.), px(theme.typography.text_sm), px(8.)),
            ComponentSize::Medium => (px(36.), px(theme.typography.text_md), px(11.)),
            ComponentSize::Large => (px(48.), px(theme.typography.text_lg), px(14.)),
        };
        let accent = self.accent();
        let initials = self.derived_initials();
        let status = self.status;

        let mut avatar = div()
            .size(dimension)
            .flex()
            .items_center()
            .justify_center()
            .bg(accent)
            .text_color(gpui::white())
            .text_size(text_size)
            .child(initials);

        avatar = match self.shape {
            AvatarShape::Circle => avatar.rounded_full(),
            AvatarShape::Rounded => avatar.rounded(px(theme.radius.md)),
        };

        let mut root = div().relative().child(avatar);

        if let Some(status) = status {
            root = root.child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .size(dot)
                    .rounded_full()
                    .border_2()
                    .border_color(theme.background())
                    .bg(status_color(status, theme)),
            );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::Avatar;

    #[test]
    fn derives_two_uppercase_initials() {
        assert_eq!(
            Avatar::new("Ada Lovelace").derived_initials().as_ref(),
            "AL"
        );
    }

    #[test]
    fn derives_single_initial_from_one_word() {
        assert_eq!(Avatar::new("guic").derived_initials().as_ref(), "G");
    }

    #[test]
    fn falls_back_when_label_is_blank() {
        assert_eq!(Avatar::new("   ").derived_initials().as_ref(), "?");
    }

    #[test]
    fn honors_explicit_initials_override() {
        assert_eq!(
            Avatar::new("Ada Lovelace")
                .initials("AD")
                .derived_initials()
                .as_ref(),
            "AD"
        );
    }

    #[test]
    fn accent_is_stable_for_same_label() {
        assert_eq!(Avatar::new("Grace").accent(), Avatar::new("Grace").accent());
    }
}
