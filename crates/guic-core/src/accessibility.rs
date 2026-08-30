use gpui::{SharedString, Toggled};
use std::hash::{Hash, Hasher};

/// Accessibility metadata exposed by interactive components.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessibilityProps {
    /// The semantic role of the element.
    pub role: Role,
    /// The primary label announced for the element.
    pub label: Option<SharedString>,
    /// A longer description announced for the element.
    pub description: Option<SharedString>,
    /// Whether the element is disabled.
    pub disabled: bool,
    /// Whether the element is selected.
    pub selected: Option<bool>,
    /// Whether the element is expanded.
    pub expanded: Option<bool>,
    /// Whether the element is checked.
    pub checked: Option<bool>,
    /// Current numeric value for range and progress controls.
    pub numeric_value: Option<f64>,
    /// Minimum numeric value for range controls.
    pub min_numeric_value: Option<f64>,
    /// Maximum numeric value for range controls.
    pub max_numeric_value: Option<f64>,
}

impl Hash for AccessibilityProps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let numeric_bits = |value: f64| {
            if value == 0.0 { 0 } else { value.to_bits() }
        };
        self.role.hash(state);
        self.label.hash(state);
        self.description.hash(state);
        self.disabled.hash(state);
        self.selected.hash(state);
        self.expanded.hash(state);
        self.checked.hash(state);
        self.numeric_value.map(numeric_bits).hash(state);
        self.min_numeric_value.map(numeric_bits).hash(state);
        self.max_numeric_value.map(numeric_bits).hash(state);
    }
}

impl AccessibilityProps {
    /// Creates accessibility metadata for a role.
    #[must_use]
    pub fn new(role: Role) -> Self {
        Self {
            role,
            ..Self::default()
        }
    }

    /// Sets the accessible label.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the accessible description metadata.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the disabled state metadata.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the selected state.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Sets the expanded state.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Sets the checked state.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Sets the current numeric value.
    #[must_use]
    pub fn numeric_value(mut self, value: f64) -> Self {
        self.numeric_value = value.is_finite().then_some(value);
        self
    }

    /// Sets the inclusive numeric range.
    #[must_use]
    pub fn numeric_range(mut self, min: f64, max: f64) -> Self {
        if min.is_finite() && max.is_finite() && min <= max {
            self.min_numeric_value = Some(min);
            self.max_numeric_value = Some(max);
        }
        self
    }

    /// Returns true when at least one platform-backed state is set.
    #[must_use]
    pub fn has_platform_state(&self) -> bool {
        self.label.is_some()
            || self.description.is_some()
            || self.selected.is_some()
            || self.expanded.is_some()
            || self.checked.is_some()
            || self.numeric_value.is_some()
            || self.min_numeric_value.is_some()
            || self.max_numeric_value.is_some()
            || self.role != Role::Generic
    }
}

/// Semantic accessibility roles recognized by GUIC.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Role {
    /// A semantic group of related controls or content.
    Group,
    /// A button.
    Button,
    /// A navigational link.
    Link,
    /// A checkbox.
    Checkbox,
    /// A radio control.
    Radio,
    /// A text input.
    TextInput,
    /// A slider.
    Slider,
    /// A binary switch.
    Switch,
    /// A numeric spin button.
    SpinButton,
    /// A progress indicator.
    ProgressIndicator,
    /// A tab.
    Tab,
    /// A tab panel.
    TabPanel,
    /// A dialog.
    Dialog,
    /// A dialog that requires an immediate response.
    AlertDialog,
    /// An important message that should be announced when it appears.
    Alert,
    /// A non-interrupting status update.
    Status,
    /// Supplementary text shown for another element.
    Tooltip,
    /// An image with a meaningful accessible label.
    Image,
    /// A menu.
    Menu,
    /// A menu item.
    MenuItem,
    /// A list box.
    ListBox,
    /// An option inside a list box.
    Option,
    /// A tree.
    Tree,
    /// A tree item.
    TreeItem,
    /// A table.
    Table,
    /// A row.
    Row,
    /// A cell.
    Cell,
    /// A default fallback role.
    #[default]
    Generic,
}

impl Role {
    /// Converts this role into the GPUI/accesskit role used by platform nodes.
    #[must_use]
    pub fn to_gpui(self) -> gpui::Role {
        match self {
            Self::Group => gpui::Role::Group,
            Self::Button => gpui::Role::Button,
            Self::Link => gpui::Role::Link,
            Self::Checkbox => gpui::Role::CheckBox,
            Self::Radio => gpui::Role::RadioButton,
            Self::TextInput => gpui::Role::TextInput,
            Self::Slider => gpui::Role::Slider,
            Self::Switch => gpui::Role::Switch,
            Self::SpinButton => gpui::Role::SpinButton,
            Self::ProgressIndicator => gpui::Role::ProgressIndicator,
            Self::Tab => gpui::Role::Tab,
            Self::TabPanel => gpui::Role::TabPanel,
            Self::Dialog => gpui::Role::Dialog,
            Self::AlertDialog => gpui::Role::AlertDialog,
            Self::Alert => gpui::Role::Alert,
            Self::Status => gpui::Role::Status,
            Self::Tooltip => gpui::Role::Tooltip,
            Self::Image => gpui::Role::Image,
            Self::Menu => gpui::Role::Menu,
            Self::MenuItem => gpui::Role::MenuItem,
            Self::ListBox => gpui::Role::ListBox,
            Self::Option => gpui::Role::ListBoxOption,
            Self::Tree => gpui::Role::Tree,
            Self::TreeItem => gpui::Role::TreeItem,
            Self::Table => gpui::Role::Table,
            Self::Row => gpui::Role::Row,
            Self::Cell => gpui::Role::Cell,
            Self::Generic => gpui::Role::GenericContainer,
        }
    }
}

/// Applies GUIC accessibility metadata to a GPUI stateful element.
pub trait AccessibilityElementExt: Sized {
    /// Applies role, label, and supported state metadata to the platform node.
    fn accessibility(self, props: AccessibilityProps) -> Self;
}

impl<T> AccessibilityElementExt for T
where
    T: gpui::StatefulInteractiveElement,
{
    fn accessibility(mut self, props: AccessibilityProps) -> Self {
        self = self.role(props.role.to_gpui());
        if let Some(label) = props.label {
            self = self.aria_label(label);
        }
        if let Some(description) = props.description {
            self = self.aria_description(description);
        }
        if let Some(selected) = props.selected {
            self = self.aria_selected(selected);
        }
        if let Some(expanded) = props.expanded {
            self = self.aria_expanded(expanded);
        }
        if let Some(checked) = props.checked {
            self = self.aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if let Some(value) = props.numeric_value {
            self = self.aria_numeric_value(value);
        }
        if let Some(value) = props.min_numeric_value {
            self = self.aria_min_numeric_value(value);
        }
        if let Some(value) = props.max_numeric_value {
            self = self.aria_max_numeric_value(value);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{AccessibilityProps, Role};

    #[test]
    fn roles_map_to_platform_roles() {
        assert_eq!(Role::Button.to_gpui(), gpui::Role::Button);
        assert_eq!(Role::Link.to_gpui(), gpui::Role::Link);
        assert_eq!(Role::Checkbox.to_gpui(), gpui::Role::CheckBox);
        assert_eq!(Role::Option.to_gpui(), gpui::Role::ListBoxOption);
        assert_eq!(Role::Switch.to_gpui(), gpui::Role::Switch);
        assert_eq!(Role::SpinButton.to_gpui(), gpui::Role::SpinButton);
        assert_eq!(Role::AlertDialog.to_gpui(), gpui::Role::AlertDialog);
        assert_eq!(Role::Alert.to_gpui(), gpui::Role::Alert);
        assert_eq!(Role::Status.to_gpui(), gpui::Role::Status);
        assert_eq!(Role::Tooltip.to_gpui(), gpui::Role::Tooltip);
        assert_eq!(Role::Image.to_gpui(), gpui::Role::Image);
        assert_eq!(
            Role::ProgressIndicator.to_gpui(),
            gpui::Role::ProgressIndicator
        );
    }

    #[test]
    fn platform_state_detection_tracks_supported_fields() {
        assert!(!AccessibilityProps::default().has_platform_state());
        assert!(
            AccessibilityProps::new(Role::Checkbox)
                .label("Accept")
                .checked(true)
                .has_platform_state()
        );
        assert!(
            AccessibilityProps::default()
                .numeric_value(50.0)
                .numeric_range(0.0, 100.0)
                .has_platform_state()
        );
        assert!(
            AccessibilityProps::default()
                .description("Additional context")
                .has_platform_state()
        );
        assert_eq!(
            AccessibilityProps::default()
                .numeric_value(f64::NAN)
                .numeric_value,
            None
        );
    }
}
