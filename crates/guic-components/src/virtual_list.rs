use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, Styled as _,
    UniformListScrollHandle, Window, div, uniform_list,
};
use std::{ops::Range, rc::Rc};

type RenderRangeFn = dyn Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement>;

/// A lightweight wrapper around GPUI's uniform list.
#[derive(gpui::IntoElement)]
pub struct VirtualList {
    id: gpui::SharedString,
    item_count: usize,
    render_range: Rc<RenderRangeFn>,
    height: gpui::Pixels,
    scroll_handle: Option<UniformListScrollHandle>,
}

impl VirtualList {
    /// Creates a new uniform-height virtual list.
    #[must_use]
    pub fn new(
        id: impl Into<gpui::SharedString>,
        item_count: usize,
        render_range: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            item_count,
            render_range: Rc::new(render_range),
            height: gpui::px(240.0),
            scroll_handle: None,
        }
    }

    /// Sets the list height.
    #[must_use]
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = height;
        self
    }

    /// Tracks list scrolling with a GPUI handle.
    #[must_use]
    pub fn track_scroll(mut self, scroll_handle: UniformListScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle);
        self
    }
}

impl RenderOnce for VirtualList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let render_range = self.render_range.clone();
        let mut list = uniform_list(self.id, self.item_count, move |range, window, cx| {
            (render_range)(range, window, cx)
        })
        .h(self.height);

        if let Some(scroll_handle) = self.scroll_handle {
            list = list.track_scroll(&scroll_handle);
        }

        div().w_full().child(list)
    }
}

/// Immutable layout metrics for a uniform-height virtual list.
///
/// These metrics let host code compute the visible window for a large dataset
/// without instantiating GPUI elements, which is useful for subsystems such as
/// [`crate::DataTable`] and [`crate::TreeView`] that own their own scroll state.
///
/// # Example
///
/// ```
/// use guic_components::VirtualListMetrics;
///
/// let metrics = VirtualListMetrics::new(20.0, 100.0, 2, 50);
/// assert_eq!(metrics.visible_range(0.0), 0..9);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualListMetrics {
    item_height: f32,
    viewport_height: f32,
    overscan: usize,
    item_count: usize,
}

impl VirtualListMetrics {
    /// Creates a new metrics descriptor.
    #[must_use]
    pub fn new(item_height: f32, viewport_height: f32, overscan: usize, item_count: usize) -> Self {
        Self {
            item_height: if item_height.is_finite() {
                item_height.max(1.0)
            } else {
                1.0
            },
            viewport_height: if viewport_height.is_finite() {
                viewport_height.max(0.0)
            } else {
                0.0
            },
            overscan,
            item_count,
        }
    }

    /// Returns the visible item range for the given scroll offset.
    #[must_use]
    pub fn visible_range(&self, scroll_offset: f32) -> Range<usize> {
        if self.item_count == 0 {
            return 0..0;
        }

        let scroll_offset = if scroll_offset.is_finite() {
            scroll_offset.max(0.0)
        } else {
            0.0
        };
        let start = (scroll_offset / self.item_height).floor() as usize;
        let visible = (self.viewport_height / self.item_height).ceil() as usize;
        let start = start.saturating_sub(self.overscan);
        let end = start
            .saturating_add(visible)
            .saturating_add(self.overscan.saturating_mul(2))
            .min(self.item_count);
        start..end
    }

    /// Returns the total content height.
    #[must_use]
    pub fn total_height(&self) -> f32 {
        self.item_height * self.item_count as f32
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::VirtualListMetrics;

    #[test]
    fn computes_virtual_list_ranges() {
        let metrics = VirtualListMetrics::new(20.0, 100.0, 2, 50);
        assert_eq!(metrics.visible_range(0.0), 0..9);
        assert_eq!(metrics.visible_range(200.0), 8..17);
    }

    #[test]
    fn empty_list_has_empty_range() {
        let metrics = VirtualListMetrics::new(20.0, 100.0, 2, 0);
        assert_eq!(metrics.visible_range(0.0), 0..0);
        assert_eq!(metrics.total_height(), 0.0);
    }

    #[test]
    fn extreme_overscan_does_not_overflow() {
        let metrics = VirtualListMetrics::new(20.0, 100.0, usize::MAX, 50);
        assert_eq!(metrics.visible_range(200.0), 0..50);
    }

    #[test]
    fn non_finite_metrics_stay_bounded() {
        let metrics = VirtualListMetrics::new(f32::INFINITY, f32::NAN, 2, 50);
        assert_eq!(metrics.visible_range(f32::INFINITY), 0..4);
        assert_eq!(metrics.total_height(), 50.0);
    }
}
