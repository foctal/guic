use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    App, ClickEvent, Empty, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, Styled as _, Window, div, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// Severity levels for a [`Toast`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToastVariant {
    /// Neutral informational toast.
    #[default]
    Info,
    /// Positive confirmation toast.
    Success,
    /// Cautionary toast.
    Warning,
    /// Error toast.
    Danger,
}

/// Where a [`ToastStack`] anchors its toasts within the window.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToastPlacement {
    /// Top-leading corner.
    TopLeft,
    /// Top-trailing corner.
    #[default]
    TopRight,
    /// Bottom-leading corner.
    BottomLeft,
    /// Bottom-trailing corner.
    BottomRight,
}

/// A single transient notification card.
///
/// `Toast` is presentational and host-managed: the host owns the list of active
/// toasts and removes one in response to [`Toast::on_close`]. Render toasts
/// through a [`ToastStack`] so they are positioned and layered correctly.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Toast, ToastVariant};
///
/// Toast::new("saved", "Changes saved")
///     .variant(ToastVariant::Success)
///     .description("Your workspace is up to date.")
///     .on_close(|_, _, _| { /* drop from the list */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Toast {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    variant: ToastVariant,
    on_close: Option<ClickHandler>,
}

impl Toast {
    /// Creates a new toast with a stable id and a title.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            variant: ToastVariant::Info,
            on_close: None,
        }
    }

    /// Sets a supporting description rendered beneath the title.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the toast variant.
    #[must_use]
    pub fn variant(mut self, variant: ToastVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Registers a close handler and enables the close affordance.
    #[must_use]
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }

    fn accent(&self, theme: &Theme) -> Hsla {
        match self.variant {
            ToastVariant::Info => theme.info(),
            ToastVariant::Success => theme.success(),
            ToastVariant::Warning => theme.warning(),
            ToastVariant::Danger => theme.danger(),
        }
    }

    fn icon_name(&self) -> IconName {
        match self.variant {
            ToastVariant::Info => IconName::Info,
            ToastVariant::Success => IconName::CheckCircle,
            ToastVariant::Warning => IconName::AlertTriangle,
            ToastVariant::Danger => IconName::XCircle,
        }
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let accent = self.accent(theme);
        let icon_name = self.icon_name();
        let selector = format!("guic-toast-{}", self.id);
        let accessibility = AccessibilityProps::new(match self.variant {
            ToastVariant::Danger | ToastVariant::Warning => Role::Alert,
            ToastVariant::Info | ToastVariant::Success => Role::Status,
        })
        .label(self.title.clone())
        .description(self.description.clone().unwrap_or_default());

        let mut body = div().flex_1().flex().flex_col().gap_0p5().child(
            div()
                .text_size(px(theme.typography.text_md))
                .text_color(theme.foreground())
                .child(self.title),
        );
        if let Some(description) = self.description {
            body = body.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.muted_foreground())
                    .child(description),
            );
        }

        div()
            .id(self.id.clone())
            .accessibility(accessibility)
            .debug_selector(move || selector.clone())
            .w(px(320.0))
            .max_w_full()
            .flex()
            .items_start()
            .gap_3()
            .px(px(theme.spacing.x4))
            .py(px(theme.spacing.x3))
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .border_l_4()
            .bg(theme.background())
            .shadow_lg()
            .child(Icon::new(icon_name).color(accent).decorative(true))
            .child(body)
            .child(if self.on_close.is_some() {
                IconButton::new(IconName::X)
                    .variant(ButtonVariant::Ghost)
                    .size(ComponentSize::Small)
                    .label("Dismiss notification")
                    .on_click_option(self.on_close)
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
    }
}

/// A positioned, layered container for active [`Toast`]s.
///
/// The host owns the list of toasts and their lifecycle (timeouts, dismissal).
/// `ToastStack` renders them in a window corner above all other content.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Toast, ToastPlacement, ToastStack};
///
/// ToastStack::new("app-toasts")
///     .placement(ToastPlacement::BottomRight)
///     .toasts(vec![Toast::new("saved", "Saved")]);
/// ```
#[derive(gpui::IntoElement)]
pub struct ToastStack {
    id: SharedString,
    placement: ToastPlacement,
    toasts: Vec<Toast>,
}

impl ToastStack {
    /// Creates a new, empty toast stack.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            placement: ToastPlacement::default(),
            toasts: Vec::new(),
        }
    }

    /// Sets the window corner the stack anchors to.
    #[must_use]
    pub fn placement(mut self, placement: ToastPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Sets the active toasts.
    #[must_use]
    pub fn toasts(mut self, toasts: Vec<Toast>) -> Self {
        self.toasts = toasts;
        self
    }
}

impl RenderOnce for ToastStack {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.toasts.is_empty() {
            return Empty.into_any_element();
        }

        let theme = Theme::global(cx);
        let (align_top, align_right) = match self.placement {
            ToastPlacement::TopLeft => (true, false),
            ToastPlacement::TopRight => (true, true),
            ToastPlacement::BottomLeft => (false, false),
            ToastPlacement::BottomRight => (false, true),
        };

        let mut container = div()
            .id(self.id.clone())
            .absolute()
            .inset(px(theme.spacing.x4))
            .flex()
            .flex_col()
            .gap(px(theme.spacing.x3));
        container = if align_top {
            container.justify_start()
        } else {
            container.justify_end()
        };
        container = if align_right {
            container.items_end()
        } else {
            container.items_start()
        };

        for toast in self.toasts {
            container = container.child(toast);
        }

        overlay_portal(
            div().absolute().inset_0().child(container),
            OverlayPriority::NOTIFICATION,
        )
    }
}
