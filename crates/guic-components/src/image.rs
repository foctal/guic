use crate::Label;
use gpui::{
    App, ImageSource, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    RenderOnce, SharedString, Styled as _, StyledImage as _, Window, div, img, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;

/// Image scaling behavior for [`Image`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImageFit {
    /// Preserve aspect ratio and fit the whole image inside the frame.
    #[default]
    Contain,
    /// Preserve aspect ratio and cover the full frame.
    Cover,
    /// Stretch the image to fill the frame.
    Fill,
    /// Render at intrinsic size inside the frame.
    None,
    /// Scale down only when the intrinsic image is larger than the frame.
    ScaleDown,
}

impl From<ImageFit> for ObjectFit {
    fn from(value: ImageFit) -> Self {
        match value {
            ImageFit::Contain => ObjectFit::Contain,
            ImageFit::Cover => ObjectFit::Cover,
            ImageFit::Fill => ObjectFit::Fill,
            ImageFit::None => ObjectFit::None,
            ImageFit::ScaleDown => ObjectFit::ScaleDown,
        }
    }
}

/// A framed image surface with loading and fallback states.
///
/// `Image` wraps GPUI's native image element with token-driven framing and a
/// small public API for common application imagery such as avatars, previews,
/// thumbnails, and documentation screenshots.
#[derive(gpui::IntoElement)]
pub struct Image {
    id: SharedString,
    source: ImageSource,
    alt: SharedString,
    fit: ImageFit,
    width: Option<f32>,
    height: Option<f32>,
    aspect_ratio: Option<f32>,
    rounded: bool,
}

impl Image {
    /// Creates a new image from a GPUI-compatible image source.
    #[must_use]
    pub fn new(source: impl Into<ImageSource>) -> Self {
        Self {
            id: SharedString::from("guic-image"),
            source: source.into(),
            alt: SharedString::default(),
            fit: ImageFit::Contain,
            width: None,
            height: None,
            aspect_ratio: None,
            rounded: true,
        }
    }

    /// Sets a stable element identifier.
    ///
    /// Use a distinct identifier when multiple images are rendered in the same
    /// view so their accessibility nodes remain stable across renders.
    #[must_use]
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets alternate text used in loading and fallback states.
    #[must_use]
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = alt.into();
        self
    }

    /// Sets image scaling behavior.
    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets an explicit width in logical pixels.
    ///
    /// Non-finite values are ignored.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.width = Some(width.max(1.0));
        }
        self
    }

    /// Sets an explicit height in logical pixels.
    ///
    /// Non-finite values are ignored.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.height = Some(height.max(1.0));
        }
        self
    }

    /// Sets a fixed aspect ratio used when no explicit height is provided.
    ///
    /// Non-finite values are ignored.
    #[must_use]
    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        if aspect_ratio.is_finite() {
            self.aspect_ratio = Some(aspect_ratio.max(0.1));
        }
        self
    }

    /// Sets whether the image frame uses the theme radius.
    #[must_use]
    pub fn rounded(mut self, rounded: bool) -> Self {
        self.rounded = rounded;
        self
    }
}

impl RenderOnce for Image {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let alt = self.alt.clone();
        let loading_alt = self.alt.clone();
        let accessibility = if self.alt.is_empty() {
            AccessibilityProps::new(Role::Image)
        } else {
            AccessibilityProps::new(Role::Image).label(self.alt.clone())
        };
        let mut frame = div()
            .id(self.id)
            .accessibility(accessibility)
            .debug_selector(|| "guic-image-frame".to_owned())
            .overflow_hidden()
            .border_1()
            .border_color(theme.border())
            .bg(theme.secondary().opacity(0.08));

        if self.rounded {
            frame = frame.rounded(px(theme.radius.md));
        }
        if let Some(width) = self.width {
            frame = frame.w(px(width));
        } else {
            frame = frame.w_full();
        }
        if let Some(height) = self.height {
            frame = frame.h(px(height));
        } else if let Some(aspect_ratio) = self.aspect_ratio {
            frame = frame.aspect_ratio(aspect_ratio);
        }

        frame.child(
            img(self.source)
                .size_full()
                .object_fit(self.fit.into())
                .with_loading(move || image_state(loading_alt.clone(), "Loading image"))
                .with_fallback(move || image_state(alt.clone(), "Image unavailable")),
        )
    }
}

fn image_state(alt: SharedString, label: &'static str) -> gpui::AnyElement {
    let text = if alt.is_empty() {
        SharedString::from(label)
    } else {
        alt
    };
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_3()
        .child(Label::new(text).muted(true))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Image, ImageFit};
    use gpui::{Context, IntoElement, ParentElement as _, Render, TestAppContext, Window, div, px};

    struct ImageHarness;

    impl Render for ImageHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(
                Image::new("asset://missing-audit-image.png")
                    .id("audit-image")
                    .alt("Audit preview")
                    .width(200.0)
                    .aspect_ratio(2.0),
            )
        }
    }

    #[test]
    fn image_builder_clamps_dimensions() {
        let image = Image::new("asset://preview.png")
            .alt("Preview")
            .width(0.0)
            .height(-10.0)
            .aspect_ratio(0.0)
            .fit(ImageFit::Cover)
            .rounded(false);

        assert_eq!(image.alt, "Preview");
        assert_eq!(image.width, Some(1.0));
        assert_eq!(image.height, Some(1.0));
        assert_eq!(image.aspect_ratio, Some(0.1));
        assert_eq!(image.fit, ImageFit::Cover);
        assert!(!image.rounded);

        let non_finite = Image::new("asset://preview.png")
            .width(f32::INFINITY)
            .height(f32::NAN)
            .aspect_ratio(f32::INFINITY);
        assert_eq!(non_finite.width, None);
        assert_eq!(non_finite.height, None);
        assert_eq!(non_finite.aspect_ratio, None);
    }

    #[gpui::test]
    fn aspect_ratio_controls_rendered_frame_geometry(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
        let (_view, cx) = cx.add_window_view(|_, _| ImageHarness);
        let bounds = cx
            .debug_bounds("guic-image-frame")
            .expect("image frame should render");
        assert_eq!(bounds.size.width, px(200.0));
        assert_eq!(bounds.size.height, px(100.0));
    }
}
