use crate::{Button, ButtonVariant, ComponentSize, IndexHandler};
use gpui::{
    App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;
use std::rc::Rc;

/// A host-managed page navigation control.
///
/// `Paginator` renders previous/next controls and a truncated run of page
/// buttons (with ellipses) around the current page. The host owns the current
/// page index and reacts to [`Paginator::on_select`].
///
/// # Example
///
/// ```no_run
/// use guic_components::Paginator;
///
/// Paginator::new("results")
///     .page_count(12)
///     .page(3)
///     .on_select(|page, _, _| { /* load that page */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Paginator {
    id: SharedString,
    page: usize,
    page_count: usize,
    sibling_count: usize,
    size: ComponentSize,
    on_select: Option<IndexHandler>,
}

impl Paginator {
    /// Creates a new paginator with a single page.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            page: 0,
            page_count: 1,
            sibling_count: 1,
            size: ComponentSize::Medium,
            on_select: None,
        }
    }

    /// Sets the total number of pages.
    #[must_use]
    pub fn page_count(mut self, page_count: usize) -> Self {
        self.page_count = page_count.max(1);
        self
    }

    /// Derives the page count from a total item count and page size.
    #[must_use]
    pub fn from_total(mut self, total_items: usize, page_size: usize) -> Self {
        let page_size = page_size.max(1);
        self.page_count = total_items.div_ceil(page_size).max(1);
        self
    }

    /// Sets the current (zero-based) page index.
    #[must_use]
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// Sets how many page buttons to show on each side of the current page.
    #[must_use]
    pub fn sibling_count(mut self, sibling_count: usize) -> Self {
        self.sibling_count = sibling_count;
        self
    }

    /// Sets the control size.
    #[must_use]
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Registers a handler invoked with the selected zero-based page index.
    #[must_use]
    pub fn on_select(
        mut self,
        on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }

    /// The clamped current page.
    fn current(&self) -> usize {
        self.page.min(self.page_count - 1)
    }
}

/// Computes the page slots to render. `None` is an ellipsis gap.
///
/// Page `0` and the last page are always shown, along with `sibling_count`
/// pages on each side of `current`.
fn visible_pages(current: usize, page_count: usize, sibling_count: usize) -> Vec<Option<usize>> {
    if page_count == 0 {
        return Vec::new();
    }
    let last = page_count - 1;
    let window_start = current.saturating_sub(sibling_count);
    let window_end = current.saturating_add(sibling_count).min(last);

    let mut wanted: Vec<usize> = Vec::new();
    wanted.push(0);
    for page in window_start..=window_end {
        wanted.push(page);
    }
    wanted.push(last);
    wanted.sort_unstable();
    wanted.dedup();

    let mut slots: Vec<Option<usize>> = Vec::new();
    let mut previous: Option<usize> = None;
    for page in wanted {
        if let Some(prev) = previous
            && page > prev + 1
        {
            slots.push(None);
        }
        slots.push(Some(page));
        previous = Some(page);
    }
    slots
}

impl RenderOnce for Paginator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let current = self.current();
        let page_count = self.page_count;
        let (button_size, cell_dim) = match self.size {
            ComponentSize::Small => (ComponentSize::Small, px(26.0)),
            ComponentSize::Medium => (ComponentSize::Small, px(28.0)),
            ComponentSize::Large => (ComponentSize::Medium, px(34.0)),
        };

        let emit = |on_select: &Option<IndexHandler>, target: usize| {
            let handler = on_select.clone();
            move |_event: &ClickEvent, window: &mut Window, cx: &mut App| {
                if let Some(handler) = handler.as_ref() {
                    handler(&target, window, cx);
                }
            }
        };

        let prev = {
            let button = Button::new("Prev")
                .variant(ButtonVariant::Secondary)
                .size(button_size)
                .disabled(current == 0);
            if current == 0 {
                button
            } else {
                button.on_click(emit(&self.on_select, current - 1))
            }
        };

        let next = {
            let button = Button::new("Next")
                .variant(ButtonVariant::Secondary)
                .size(button_size)
                .disabled(current + 1 >= page_count);
            if current + 1 >= page_count {
                button
            } else {
                button.on_click(emit(&self.on_select, current + 1))
            }
        };

        let mut row = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap_1()
            .child(prev);

        for slot in visible_pages(current, page_count, self.sibling_count) {
            match slot {
                None => {
                    row = row.child(
                        div()
                            .px(px(theme.spacing.x2))
                            .text_color(theme.muted_foreground())
                            .child("…"),
                    );
                }
                Some(page) => {
                    let is_current = page == current;
                    let label = SharedString::from((page + 1).to_string());
                    let cell = div()
                        .id(SharedString::from(format!("{}-page-{page}", self.id)))
                        .accessibility(
                            AccessibilityProps::new(Role::Button)
                                .label(format!("Page {}", page + 1))
                                .selected(is_current),
                        )
                        .debug_selector({
                            let id = self.id.clone();
                            move || format!("guic-paginator-{id}-page-{page}")
                        })
                        .min_w(cell_dim)
                        .h(cell_dim)
                        .px(px(theme.spacing.x2))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(theme.radius.md))
                        .border_1()
                        .border_color(if is_current {
                            theme.primary()
                        } else {
                            theme.border()
                        })
                        .bg(if is_current {
                            theme.primary()
                        } else {
                            theme.background()
                        })
                        .text_size(px(theme.typography.text_sm))
                        .text_color(if is_current {
                            gpui::white()
                        } else {
                            theme.foreground()
                        })
                        .child(label);

                    row = if is_current {
                        row.child(cell)
                    } else {
                        let keyboard_handler = self.on_select.clone();
                        row.child(
                            cell.tab_index(0)
                                .key_context("GuicPaginatorPage")
                                .cursor_pointer()
                                .hover({
                                    let hover = theme.secondary().opacity(0.3);
                                    move |style: gpui::StyleRefinement| style.bg(hover)
                                })
                                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        if let Some(handler) = keyboard_handler.as_ref() {
                                            handler(&page, window, cx);
                                        }
                                        cx.stop_propagation();
                                    }
                                })
                                .on_click(emit(&self.on_select, page)),
                        )
                    };
                }
            }
        }

        row.child(next)
    }
}

#[cfg(test)]
mod tests {
    use super::visible_pages;

    #[test]
    fn small_counts_show_every_page() {
        assert_eq!(
            visible_pages(0, 3, 1),
            vec![Some(0), Some(1), Some(2)],
            "no ellipsis when all pages fit"
        );
    }

    #[test]
    fn middle_page_truncates_both_sides() {
        assert_eq!(
            visible_pages(5, 11, 1),
            vec![Some(0), None, Some(4), Some(5), Some(6), None, Some(10)]
        );
    }

    #[test]
    fn near_start_only_truncates_the_end() {
        assert_eq!(
            visible_pages(1, 11, 1),
            vec![Some(0), Some(1), Some(2), None, Some(10)]
        );
    }

    #[test]
    fn near_end_only_truncates_the_start() {
        assert_eq!(
            visible_pages(9, 11, 1),
            vec![Some(0), None, Some(8), Some(9), Some(10)]
        );
    }

    #[test]
    fn adjacent_gap_is_filled_not_ellipsized() {
        // Page 2 is adjacent to both 0..1 and the last page region, so the gap
        // collapses to a literal page rather than an ellipsis.
        assert_eq!(
            visible_pages(2, 5, 1),
            vec![Some(0), Some(1), Some(2), Some(3), Some(4)]
        );
    }

    #[test]
    fn extreme_sibling_count_does_not_overflow() {
        assert_eq!(
            visible_pages(1, 3, usize::MAX),
            vec![Some(0), Some(1), Some(2)]
        );
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::Paginator;
    use gpui::{
        Context, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext, Window, div,
    };

    struct PaginatorHarness {
        page: usize,
    }

    impl Render for PaginatorHarness {
        fn render(
            &mut self,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            div().size_full().p_4().child(
                Paginator::new("results")
                    .page_count(10)
                    .page(self.page)
                    .on_select(cx.listener(|this, page: &usize, _, cx| {
                        this.page = *page;
                        cx.notify();
                    })),
            )
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            guic_core::init(cx);
            guic_tokens::init(cx);
            crate::init(cx);
        });
    }

    #[gpui::test]
    fn page_button_click_selects_page(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| PaginatorHarness { page: 0 });

        // The last page is always shown; jump to it.
        let last = cx
            .debug_bounds("guic-paginator-results-page-9")
            .expect("last page button should be present");
        cx.simulate_click(last.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.page, 9));
    }

    #[gpui::test]
    fn prev_and_next_buttons_step_pages(cx: &mut TestAppContext) {
        init(cx);
        let (view, cx) = cx.add_window_view(|_, _| PaginatorHarness { page: 4 });

        let next = cx
            .debug_bounds("guic-button-Next")
            .expect("next button should be present");
        cx.simulate_click(next.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.page, 5));

        let prev = cx
            .debug_bounds("guic-button-Prev")
            .expect("prev button should be present");
        cx.simulate_click(prev.center(), Modifiers::none());
        view.update(cx, |view, _| assert_eq!(view.page, 4));
    }
}
