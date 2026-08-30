use crate::{ButtonVariant, ClickHandler, ComponentSize, IconButton};
use gpui::{
    AnyElement, App, ClickEvent, Empty, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{
    AccessibilityElementExt as _, AccessibilityProps, OverlayPriority, Role, overlay_portal,
};
use guic_icons::IconName;
use guic_tokens::Theme;
use std::rc::Rc;

/// The edge a [`Drawer`] slides in from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DrawerSide {
    /// Anchored to the left edge (full height).
    #[default]
    Left,
    /// Anchored to the right edge (full height).
    Right,
    /// Anchored to the top edge (full width).
    Top,
    /// Anchored to the bottom edge (full width).
    Bottom,
}

impl DrawerSide {
    fn is_horizontal(self) -> bool {
        matches!(self, DrawerSide::Left | DrawerSide::Right)
    }
}

/// A controlled edge-anchored panel that slides in over a dismiss scrim.
///
/// `Drawer` is host-managed: supply [`Drawer::open`] and react to
/// [`Drawer::on_close`] (fired by the scrim and the header close button). It is
/// the canonical surface for side navigation, detail panes, and settings sheets.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Drawer, DrawerSide, Label};
///
/// Drawer::new("details")
///     .open(true)
///     .side(DrawerSide::Right)
///     .title("Details")
///     .on_close(|_, _, _| { /* close */ })
///     .child(Label::new("Selected item"));
/// ```
#[derive(gpui::IntoElement)]
pub struct Drawer {
    id: SharedString,
    open: bool,
    side: DrawerSide,
    title: Option<SharedString>,
    size: f32,
    dismissible: bool,
    body: Vec<AnyElement>,
    footer: Option<AnyElement>,
    on_close: Option<ClickHandler>,
}

impl Drawer {
    /// Creates a new, closed drawer.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            side: DrawerSide::Left,
            title: None,
            size: 320.0,
            dismissible: true,
            body: Vec::new(),
            footer: None,
            on_close: None,
        }
    }

    /// Sets whether the drawer is visible.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Sets the edge the drawer is anchored to.
    #[must_use]
    pub fn side(mut self, side: DrawerSide) -> Self {
        self.side = side;
        self
    }

    /// Sets the drawer title rendered in the header.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the panel size in pixels (width for left/right, height for
    /// top/bottom).
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        if size.is_finite() {
            self.size = size.max(1.0);
        }
        self
    }

    /// Sets whether clicking the scrim closes the drawer.
    #[must_use]
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Appends a child element to the drawer body.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// Sets a footer region, typically containing actions.
    #[must_use]
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// Registers a close handler fired by the scrim and the header close button.
    #[must_use]
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }
}

impl RenderOnce for Drawer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.open {
            return Empty.into_any_element();
        }

        let theme = Theme::global(cx);
        let window_size = window.bounds().size;
        let side = self.side;
        let has_title = self.title.is_some();
        let accessibility_label = self
            .title
            .clone()
            .unwrap_or_else(|| SharedString::from("Drawer"));

        let mut header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px(px(theme.spacing.x4))
            .py(px(theme.spacing.x3))
            .border_b_1()
            .border_color(theme.border());
        if let Some(title) = self.title {
            header = header.child(
                div()
                    .text_size(px(theme.typography.text_lg))
                    .text_color(theme.foreground())
                    .child(title),
            );
        }
        header = header.child(
            IconButton::new(IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ComponentSize::Small)
                .label("Close drawer")
                .on_click_option(self.on_close.clone()),
        );

        let mut body = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(theme.spacing.x3))
            .px(px(theme.spacing.x4))
            .py(px(theme.spacing.x4))
            .overflow_hidden();
        for child in self.body {
            body = body.child(child);
        }

        let mut panel = div()
            .id(self.id.clone())
            .accessibility(AccessibilityProps::new(Role::Dialog).label(accessibility_label))
            .debug_selector(|| "guic-drawer-panel".to_owned())
            .absolute()
            .flex()
            .flex_col()
            .bg(theme.background())
            .border_color(theme.border())
            .shadow_xl();

        panel = if side.is_horizontal() {
            panel
                .top_0()
                .bottom_0()
                .w(px(self.size.min(f32::from(window_size.width))))
        } else {
            panel
                .left_0()
                .right_0()
                .h(px(self.size.min(f32::from(window_size.height))))
        };
        panel = match side {
            DrawerSide::Left => panel.left_0().border_r_1(),
            DrawerSide::Right => panel.right_0().border_l_1(),
            DrawerSide::Top => panel.top_0().border_b_1(),
            DrawerSide::Bottom => panel.bottom_0().border_t_1(),
        };

        if has_title || self.on_close.is_some() {
            panel = panel.child(header);
        }
        panel = panel.child(body);
        if let Some(footer) = self.footer {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(theme.spacing.x2))
                    .px(px(theme.spacing.x4))
                    .py(px(theme.spacing.x3))
                    .border_t_1()
                    .border_color(theme.border())
                    .child(footer),
            );
        }

        let scrim_id = format!("{}-scrim", self.id);
        let mut scrim = div()
            .id(SharedString::from(scrim_id))
            .debug_selector(|| "guic-drawer-scrim".to_owned())
            .absolute()
            .inset_0()
            .bg(theme.foreground().opacity(0.22));
        if self.dismissible
            && let Some(on_close) = self.on_close.clone()
        {
            scrim =
                scrim.on_click(move |event: &ClickEvent, window, cx| (on_close)(event, window, cx));
        }

        overlay_portal(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(window_size.width)
                .h(window_size.height)
                .child(scrim)
                .child(panel),
            OverlayPriority::MODAL,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Drawer, DrawerSide};

    #[test]
    fn horizontal_sides_are_detected() {
        assert!(DrawerSide::Left.is_horizontal());
        assert!(DrawerSide::Right.is_horizontal());
        assert!(!DrawerSide::Top.is_horizontal());
        assert!(!DrawerSide::Bottom.is_horizontal());
    }

    #[test]
    fn drawer_size_rejects_invalid_layout_values() {
        assert_eq!(Drawer::new("drawer").size(-20.0).size, 1.0);
        assert_eq!(Drawer::new("drawer").size(f32::INFINITY).size, 320.0);
    }
}
