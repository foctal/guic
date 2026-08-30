use crate::IndexHandler;
use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;
use std::rc::Rc;

/// A single collapsible section within an [`Accordion`].
pub struct AccordionSection {
    title: SharedString,
    expanded: bool,
    body: AnyElement,
}

impl AccordionSection {
    /// Creates a new section with a title and body content.
    #[must_use]
    pub fn new(title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        Self {
            title: title.into(),
            expanded: false,
            body: body.into_any_element(),
        }
    }

    /// Sets whether the section is expanded.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
}

/// A stacked set of collapsible sections.
///
/// Expansion is host-managed: provide each section's `expanded` flag and react
/// to [`Accordion::on_toggle`], which reports the index of the toggled section.
/// This keeps the accordion compatible with both single-open and multi-open
/// policies — the host decides.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Accordion, AccordionSection, Label};
///
/// Accordion::new("settings")
///     .section(AccordionSection::new("General", Label::new("…")).expanded(true))
///     .section(AccordionSection::new("Advanced", Label::new("…")))
///     .on_toggle(|index, _, _| { /* flip the section */ });
/// ```
#[derive(gpui::IntoElement)]
pub struct Accordion {
    id: SharedString,
    sections: Vec<AccordionSection>,
    on_toggle: Option<IndexHandler>,
}

impl Accordion {
    /// Creates a new, empty accordion.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            sections: Vec::new(),
            on_toggle: None,
        }
    }

    /// Appends a section.
    #[must_use]
    pub fn section(mut self, section: AccordionSection) -> Self {
        self.sections.push(section);
        self
    }

    /// Registers a toggle handler invoked with the toggled section's index.
    #[must_use]
    pub fn on_toggle(
        mut self,
        on_toggle: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let last = self.sections.len().saturating_sub(1);

        let mut root = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .rounded(px(theme.radius.lg))
            .border_1()
            .border_color(theme.border())
            .bg(theme.background())
            .overflow_hidden();

        for (ix, section) in self.sections.into_iter().enumerate() {
            let expanded = section.expanded;
            let on_toggle = self.on_toggle.clone();
            let header_id = SharedString::from(format!("{}-section-{}", self.id, ix));
            let selector = format!("guic-accordion-{}-section-{}", self.id, ix);

            let mut header = div()
                .id(header_id)
                .accessibility(
                    AccessibilityProps::new(Role::Button)
                        .label(section.title.clone())
                        .expanded(expanded),
                )
                .debug_selector(move || selector.clone())
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px(px(theme.spacing.x4))
                .py(px(theme.spacing.x3))
                .cursor_pointer()
                .hover(|style: gpui::StyleRefinement| style.bg(theme.secondary().opacity(0.3)))
                .child(
                    div()
                        .text_size(px(theme.typography.text_md))
                        .text_color(theme.foreground())
                        .child(section.title),
                )
                .child(
                    Icon::new(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .color(theme.muted_foreground())
                    .decorative(true),
                );

            if let Some(handler) = on_toggle {
                let keyboard_handler = handler.clone();
                header = header
                    .tab_index(0)
                    .key_context("GuicAccordionHeader")
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            keyboard_handler(&ix, window, cx);
                            cx.stop_propagation();
                        }
                    })
                    .on_click(move |_event: &ClickEvent, window, cx| {
                        handler(&ix, window, cx);
                    });
            }

            let mut item = div().flex().flex_col().child(header);

            if expanded {
                item = item.child(
                    div()
                        .px(px(theme.spacing.x4))
                        .pb(px(theme.spacing.x4))
                        .child(section.body),
                );
            }

            if ix != last {
                item = item.border_b_1().border_color(theme.border());
            }

            root = root.child(item);
        }

        root
    }
}
