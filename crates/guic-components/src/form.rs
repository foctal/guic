use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px,
};
use guic_core::{AccessibilityElementExt as _, AccessibilityProps, Role};
use guic_tokens::Theme;

/// Validation severity displayed by [`FormField`] and [`FormSummary`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ValidationSeverity {
    /// The field has no validation result.
    #[default]
    None,
    /// The field value is valid.
    Success,
    /// The field value should be reviewed but may still be submitted.
    Warning,
    /// The field value is invalid.
    Error,
}

/// A validation issue associated with a field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationIssue {
    /// Stable field identifier.
    pub field_id: SharedString,
    /// Human-readable field label.
    pub field_label: SharedString,
    /// Validation message.
    pub message: SharedString,
    /// Validation severity.
    pub severity: ValidationSeverity,
}

impl ValidationIssue {
    /// Creates an error validation issue.
    #[must_use]
    pub fn error(
        field_id: impl Into<SharedString>,
        field_label: impl Into<SharedString>,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            field_id: field_id.into(),
            field_label: field_label.into(),
            message: message.into(),
            severity: ValidationSeverity::Error,
        }
    }

    /// Changes the issue severity.
    #[must_use]
    pub fn severity(mut self, severity: ValidationSeverity) -> Self {
        self.severity = severity;
        self
    }
}

/// A vertically arranged form with content and action regions.
#[derive(gpui::IntoElement)]
pub struct Form {
    id: SharedString,
    label: Option<SharedString>,
    children: Vec<AnyElement>,
    actions: Vec<AnyElement>,
}

impl Form {
    /// Creates an empty form.
    #[must_use]
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: None,
            children: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Sets the accessible form label.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Appends form content.
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Appends an action to the bottom action row.
    #[must_use]
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for Form {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let mut root = div()
            .id(self.id)
            .accessibility(
                AccessibilityProps::new(Role::Group)
                    .label(self.label.unwrap_or_else(|| "Form".into())),
            )
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            .children(self.children);

        if !self.actions.is_empty() {
            root = root.child(
                div()
                    .w_full()
                    .pt(px(theme.spacing.x2))
                    .border_t_1()
                    .border_color(theme.border())
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .children(self.actions),
            );
        }
        root
    }
}

/// A labelled form-control layout with description and validation feedback.
#[derive(gpui::IntoElement)]
pub struct FormField {
    id: SharedString,
    label: SharedString,
    description: Option<SharedString>,
    required: bool,
    disabled: bool,
    validation: ValidationSeverity,
    message: Option<SharedString>,
    child: AnyElement,
}

impl FormField {
    /// Creates a field around one form control.
    #[must_use]
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            required: false,
            disabled: false,
            validation: ValidationSeverity::None,
            message: None,
            child: child.into_any_element(),
        }
    }

    /// Sets supporting text displayed before validation feedback.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Marks the field as required.
    #[must_use]
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Marks the field as disabled for semantic and visual purposes.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets validation severity and feedback.
    #[must_use]
    pub fn validation(
        mut self,
        severity: ValidationSeverity,
        message: impl Into<SharedString>,
    ) -> Self {
        self.validation = severity;
        self.message = Some(message.into());
        self
    }
}

impl RenderOnce for FormField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let label: SharedString = if self.required {
            format!("{} (required)", self.label).into()
        } else {
            self.label.clone()
        };
        let accessible_description = self.message.clone().or_else(|| self.description.clone());
        let mut label_row = div()
            .flex()
            .items_center()
            .gap_1()
            .text_size(px(theme.typography.text_sm))
            .text_color(if self.disabled {
                theme.muted_foreground()
            } else {
                theme.foreground()
            })
            .child(self.label);
        if self.required {
            label_row = label_row.child(div().text_color(theme.danger()).child("*"));
        }

        let mut root = div()
            .id(self.id)
            .accessibility(
                AccessibilityProps::new(Role::Group)
                    .label(label)
                    .description(accessible_description.unwrap_or_default())
                    .disabled(self.disabled),
            )
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(label_row)
            .child(self.child);

        if let Some(description) = self.description {
            root = root.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.muted_foreground())
                    .child(description),
            );
        }
        if let Some(message) = self.message {
            let color = match self.validation {
                ValidationSeverity::None => theme.muted_foreground(),
                ValidationSeverity::Success => theme.success(),
                ValidationSeverity::Warning => theme.warning(),
                ValidationSeverity::Error => theme.danger(),
            };
            root = root.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(color)
                    .child(message),
            );
        }
        root
    }
}

/// A validation summary suitable for the top of a submitted form.
#[derive(gpui::IntoElement)]
pub struct FormSummary {
    id: SharedString,
    title: SharedString,
    issues: Vec<ValidationIssue>,
}

impl FormSummary {
    /// Creates a summary from validation issues.
    #[must_use]
    pub fn new(id: impl Into<SharedString>, issues: Vec<ValidationIssue>) -> Self {
        Self {
            id: id.into(),
            title: "Review the highlighted fields".into(),
            issues,
        }
    }

    /// Sets the summary title.
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }
}

impl RenderOnce for FormSummary {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let visible = self
            .issues
            .into_iter()
            .filter(|issue| issue.severity != ValidationSeverity::None)
            .collect::<Vec<_>>();
        let mut root = div()
            .id(self.id)
            .accessibility(AccessibilityProps::new(Role::Group).label(self.title.clone()))
            .w_full()
            .rounded(px(theme.radius.md))
            .border_1()
            .border_color(theme.danger())
            .bg(theme.danger().opacity(0.08))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_color(theme.foreground())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.title),
            );
        for issue in visible {
            root = root.child(
                div()
                    .text_size(px(theme.typography.text_sm))
                    .text_color(theme.foreground())
                    .child(format!("{}: {}", issue.field_label, issue.message)),
            );
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use gpui::div;

    use super::{Form, FormField, FormSummary, ValidationIssue, ValidationSeverity};

    #[test]
    fn form_builders_track_layout_and_validation_metadata() {
        let form = Form::new("profile")
            .label("Profile")
            .child(FormField::new("name", "Name", div()).required(true))
            .action(div());
        assert_eq!(form.label.as_deref(), Some("Profile"));
        assert_eq!(form.children.len(), 1);
        assert_eq!(form.actions.len(), 1);

        let field = FormField::new("email", "Email", div())
            .description("Used for notifications")
            .validation(ValidationSeverity::Error, "Enter a valid email");
        assert_eq!(field.validation, ValidationSeverity::Error);
        assert_eq!(field.message.as_deref(), Some("Enter a valid email"));
    }

    #[test]
    fn validation_issue_defaults_to_error_and_can_change_severity() {
        let issue = ValidationIssue::error("email", "Email", "Required")
            .severity(ValidationSeverity::Warning);
        assert_eq!(issue.severity, ValidationSeverity::Warning);

        let summary = FormSummary::new("profile-errors", vec![issue]).title("Check this form");
        assert_eq!(summary.title.as_ref(), "Check this form");
        assert_eq!(summary.issues.len(), 1);
    }
}
