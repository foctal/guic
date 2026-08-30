//! Optional icon integration for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use gpui::{
    App, Bounds, Element, GlobalElementId, Hitbox, InspectorElementId, InteractiveElement,
    IntoElement, LayoutId, Pixels, RenderOnce, Role, SharedString, StyleRefinement, Styled, Window,
    px,
};
use guic_tokens::Theme;

/// A stable icon identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IconName {
    /// Informational status.
    Info,
    /// Confirmation checkmark.
    Check,
    /// Successful status.
    CheckCircle,
    /// Warning status.
    AlertTriangle,
    /// Error status.
    XCircle,
    /// Close affordance.
    X,
    /// Search affordance.
    Search,
    /// Directional chevron.
    ChevronRight,
    /// Downward directional chevron.
    ChevronDown,
    /// External link affordance.
    ExternalLink,
    /// Positive add affordance.
    Plus,
    /// Remove or collapse affordance.
    Minus,
}

impl IconName {
    /// Returns the stable embedded asset key for the icon.
    #[must_use]
    pub fn asset_path(self) -> &'static str {
        match self {
            Self::Info => "guic-icons/info.svg",
            Self::Check => "guic-icons/check.svg",
            Self::CheckCircle => "guic-icons/check-circle.svg",
            Self::AlertTriangle => "guic-icons/alert-triangle.svg",
            Self::XCircle => "guic-icons/x-circle.svg",
            Self::X => "guic-icons/x.svg",
            Self::Search => "guic-icons/search.svg",
            Self::ChevronRight => "guic-icons/chevron-right.svg",
            Self::ChevronDown => "guic-icons/chevron-down.svg",
            Self::ExternalLink => "guic-icons/external-link.svg",
            Self::Plus => "guic-icons/plus.svg",
            Self::Minus => "guic-icons/minus.svg",
        }
    }

    fn asset_bytes(self) -> &'static [u8] {
        match self {
            Self::Info => include_bytes!("../assets/info.svg"),
            Self::Check => include_bytes!("../assets/check.svg"),
            Self::CheckCircle => include_bytes!("../assets/check-circle.svg"),
            Self::AlertTriangle => include_bytes!("../assets/alert-triangle.svg"),
            Self::XCircle => include_bytes!("../assets/x-circle.svg"),
            Self::X => include_bytes!("../assets/x.svg"),
            Self::Search => include_bytes!("../assets/search.svg"),
            Self::ChevronRight => include_bytes!("../assets/chevron-right.svg"),
            Self::ChevronDown => include_bytes!("../assets/chevron-down.svg"),
            Self::ExternalLink => include_bytes!("../assets/external-link.svg"),
            Self::Plus => include_bytes!("../assets/plus.svg"),
            Self::Minus => include_bytes!("../assets/minus.svg"),
        }
    }
}

/// An SVG-backed icon element.
#[derive(gpui::IntoElement)]
pub struct Icon {
    icon: IconName,
    size: f32,
    color: Option<gpui::Hsla>,
    label: Option<SharedString>,
}

impl Icon {
    /// Creates a new icon element.
    #[must_use]
    pub fn new(icon: IconName) -> Self {
        Self {
            icon,
            size: 14.0,
            color: None,
            label: None,
        }
    }

    /// Sets the icon size in pixels.
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the icon color.
    #[must_use]
    pub fn color(mut self, color: gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Marks the icon as semantically meaningful for assistive technology.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Marks the icon as decorative so it stays out of the accessibility tree.
    #[must_use]
    pub fn decorative(mut self, decorative: bool) -> Self {
        if decorative {
            self.label = None;
        }
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let color = self.color.unwrap_or_else(|| theme.foreground());
        let label = self.label.clone();
        let icon = EmbeddedSvg::new(
            self.icon.asset_path(),
            self.icon.asset_bytes(),
            label.clone(),
        )
        .w(px(self.size))
        .h(px(self.size))
        .text_color(color);

        if let Some(label) = label {
            icon.id(format!("guic-icon-{}-{}", self.icon.asset_path(), label))
                .into_any_element()
        } else {
            icon.into_any_element()
        }
    }
}

struct EmbeddedSvg {
    interactivity: gpui::Interactivity,
    path: SharedString,
    bytes: &'static [u8],
    label: Option<SharedString>,
}

impl EmbeddedSvg {
    fn new(
        path: impl Into<SharedString>,
        bytes: &'static [u8],
        label: Option<SharedString>,
    ) -> Self {
        Self {
            interactivity: gpui::Interactivity::new(),
            path: path.into(),
            bytes,
            label,
        }
    }
}

impl Element for EmbeddedSvg {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<gpui::ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn a11y_role(&self) -> Option<Role> {
        self.label.as_ref().map(|_| Role::Image)
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        if let Some(label) = &self.label {
            node.set_label(label.to_string());
        }
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| window.request_layout(style, None, cx),
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, _, _| hitbox,
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |style, window, cx| {
                if let Some(color) = style.text.color {
                    let _ = window.paint_svg(
                        bounds,
                        self.path.clone(),
                        Some(self.bytes),
                        gpui::TransformationMatrix::default(),
                        color,
                        cx,
                    );
                }
            },
        );
    }
}

impl IntoElement for EmbeddedSvg {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for EmbeddedSvg {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for EmbeddedSvg {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        &mut self.interactivity
    }
}

#[cfg(test)]
mod tests {
    use super::{Icon, IconName};

    #[test]
    fn icon_assets_are_embedded() {
        let icons = [
            IconName::Info,
            IconName::Check,
            IconName::CheckCircle,
            IconName::AlertTriangle,
            IconName::XCircle,
            IconName::X,
            IconName::Search,
            IconName::ChevronRight,
            IconName::ChevronDown,
            IconName::ExternalLink,
            IconName::Plus,
            IconName::Minus,
        ];

        for icon in icons {
            assert!(icon.asset_path().starts_with("guic-icons/"));
            assert!(!icon.asset_bytes().is_empty());
        }
    }

    #[test]
    fn icon_labels_opt_into_semantic_accessibility() {
        let decorative = Icon::new(IconName::Info);
        let semantic = Icon::new(IconName::Info).label("Information");

        assert!(decorative.label.is_none());
        assert_eq!(semantic.label.as_deref(), Some("Information"));
    }
}
