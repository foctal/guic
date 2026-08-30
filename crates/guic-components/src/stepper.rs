use gpui::{
    App, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use guic_icons::{Icon, IconName};
use guic_tokens::Theme;

/// A single step in a [`Stepper`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    /// The step's primary label.
    pub label: SharedString,
    /// Optional supporting description rendered beneath the label.
    pub description: Option<SharedString>,
}

impl Step {
    /// Creates a new step with a label.
    #[must_use]
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }

    /// Sets a supporting description.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A horizontal progress indicator for multi-step workflows.
///
/// Steps before `active` render as completed (with a check), the `active` step
/// is emphasized, and later steps are muted. `Stepper` is presentational; the
/// host owns the active index.
///
/// # Example
///
/// ```no_run
/// use guic_components::{Step, Stepper};
///
/// Stepper::new()
///     .active(1)
///     .steps(vec![
///         Step::new("Account"),
///         Step::new("Profile"),
///         Step::new("Review"),
///     ]);
/// ```
#[derive(gpui::IntoElement)]
pub struct Stepper {
    id: SharedString,
    steps: Vec<Step>,
    active: usize,
}

impl Default for Stepper {
    fn default() -> Self {
        Self::new()
    }
}

impl Stepper {
    /// Creates a new, empty stepper.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: SharedString::from("guic-stepper"),
            steps: Vec::new(),
            active: 0,
        }
    }

    /// Sets a stable identifier for the horizontal scroll surface.
    #[must_use]
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets the steps.
    #[must_use]
    pub fn steps(mut self, steps: Vec<Step>) -> Self {
        self.steps = steps;
        self
    }

    /// Sets the active (current) step index.
    #[must_use]
    pub fn active(mut self, active: usize) -> Self {
        self.active = active;
        self
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StepState {
    Complete,
    Active,
    Upcoming,
}

impl RenderOnce for Stepper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let active = self.active;
        let total = self.steps.len();

        let mut row = div()
            .id(self.id)
            .flex()
            .items_start()
            .w_full()
            .overflow_x_scroll();

        for (ix, step) in self.steps.into_iter().enumerate() {
            let state = match ix.cmp(&active) {
                std::cmp::Ordering::Less => StepState::Complete,
                std::cmp::Ordering::Equal => StepState::Active,
                std::cmp::Ordering::Greater => StepState::Upcoming,
            };

            let marker_bg: Hsla = match state {
                StepState::Complete => theme.primary(),
                StepState::Active => theme.primary(),
                StepState::Upcoming => theme.secondary().opacity(0.5),
            };
            let marker_fg = match state {
                StepState::Complete | StepState::Active => gpui::white(),
                StepState::Upcoming => theme.muted_foreground(),
            };

            let marker = div()
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(marker_bg)
                .text_color(marker_fg)
                .text_size(px(theme.typography.text_sm));
            let marker = if state == StepState::Complete {
                marker.child(Icon::new(IconName::CheckCircle).size(14.0).color(marker_fg))
            } else {
                marker.child(SharedString::from((ix + 1).to_string()))
            };

            let mut label_col = div().flex().flex_col().gap_0p5().child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(if state == StepState::Upcoming {
                        theme.muted_foreground()
                    } else {
                        theme.foreground()
                    })
                    .child(step.label),
            );
            if let Some(description) = step.description {
                label_col = label_col.child(
                    div()
                        .text_size(px(theme.typography.text_sm))
                        .text_color(theme.muted_foreground())
                        .child(description),
                );
            }

            let step_cell = div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap_2()
                .child(marker)
                .child(label_col);

            row = row.child(step_cell);

            if ix + 1 < total {
                row = row.child(
                    div()
                        .flex_1()
                        .h(px(2.))
                        .mx_2()
                        .mt(px(11.))
                        .rounded_full()
                        .bg(if ix < active {
                            theme.primary()
                        } else {
                            theme.border()
                        }),
                );
            }
        }

        row
    }
}
