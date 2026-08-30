use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    ScrollHandle, StatefulInteractiveElement as _, Styled as _, Window, div,
};

/// A simple scroll container.
#[derive(gpui::IntoElement)]
pub struct ScrollArea {
    id: gpui::SharedString,
    child: AnyElement,
    scroll_handle: Option<ScrollHandle>,
    horizontal: bool,
    vertical: bool,
}

impl ScrollArea {
    /// Creates a new scroll area around the provided child.
    #[must_use]
    pub fn new(id: impl Into<gpui::SharedString>, child: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
            scroll_handle: None,
            horizontal: false,
            vertical: true,
        }
    }

    /// Enables or disables horizontal scrolling.
    #[must_use]
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.horizontal = horizontal;
        self
    }

    /// Enables or disables vertical scrolling.
    #[must_use]
    pub fn vertical(mut self, vertical: bool) -> Self {
        self.vertical = vertical;
        self
    }

    /// Tracks the scroll position with a GPUI handle.
    #[must_use]
    pub fn track_scroll(mut self, handle: ScrollHandle) -> Self {
        self.scroll_handle = Some(handle);
        self
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut root = div().id(self.id).size_full();

        root = match (self.horizontal, self.vertical) {
            (true, true) => root.overflow_scroll(),
            (true, false) => root.overflow_x_scroll(),
            (false, true) => root.overflow_y_scroll(),
            (false, false) => root,
        };

        if let Some(handle) = self.scroll_handle.as_ref() {
            root = root.track_scroll(handle);
        }

        root.child(self.child)
    }
}
