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
        self.disabled
            || self.label.is_some()
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
    /// A masked text input.
    PasswordInput,
    /// A multi-line text input.
    MultilineTextInput,
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
            Self::PasswordInput => gpui::Role::PasswordInput,
            Self::MultilineTextInput => gpui::Role::MultilineTextInput,
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

    /// Applies all metadata, including disabled semantics, without adding a node.
    ///
    /// Prefer this method for new elements. The original `accessibility` method
    /// retains its concrete return type for source compatibility, but GPUI 0.6
    /// cannot attach disabled semantics through its fluent API alone.
    fn accessible(self, props: AccessibilityProps) -> AccessibleElement<Self>
    where
        Self: gpui::Element,
    {
        let disabled = props.disabled;
        AccessibleElement {
            inner: self.accessibility(props),
            disabled,
        }
    }
}

impl<T> AccessibilityElementExt for T
where
    T: gpui::StatefulInteractiveElement,
{
    fn accessibility(mut self, props: AccessibilityProps) -> Self {
        if props.role != Role::Generic {
            self = self.role(props.role.to_gpui());
        }
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

/// An element that preserves layout, focus, and events while exposing disabled semantics.
pub struct AccessibleElement<E> {
    inner: E,
    disabled: bool,
}

impl<E: gpui::Element> gpui::IntoElement for AccessibleElement<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<E: gpui::InteractiveElement> gpui::InteractiveElement for AccessibleElement<E> {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.inner.interactivity()
    }
}
impl<E: gpui::StatefulInteractiveElement> gpui::StatefulInteractiveElement
    for AccessibleElement<E>
{
}
impl<E: gpui::Styled> gpui::Styled for AccessibleElement<E> {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        self.inner.style()
    }
}
impl<E: gpui::ParentElement> gpui::ParentElement for AccessibleElement<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.inner.extend(elements);
    }
}
impl<E: gpui::Element> gpui::Element for AccessibleElement<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;
    fn id(&self) -> Option<gpui::ElementId> {
        self.inner.id()
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector, bounds, layout, window, cx)
    }
    fn paint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        inspector: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        self.inner
            .paint(id, inspector, bounds, layout, prepaint, window, cx);
    }
    fn a11y_role(&self) -> Option<gpui::Role> {
        self.inner.a11y_role()
    }
    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.inner.write_a11y_info(node);
        if self.disabled {
            node.set_disabled();
        }
    }
    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut gpui::A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }
}

#[cfg(test)]
mod tests {
    use super::{AccessibilityElementExt as _, AccessibilityProps, Role};

    #[test]
    fn disabled_semantics_preserve_the_original_node() {
        use gpui::{Element as _, InteractiveElement as _, ParentElement as _, Styled as _};
        for disabled in [false, true] {
            let element = gpui::div()
                .id("control")
                .accessible(
                    AccessibilityProps::new(Role::Button)
                        .label("Save")
                        .disabled(disabled),
                )
                .p_2()
                .child("Save");
            let mut node = gpui::accesskit::Node::new(element.a11y_role().expect("role"));
            element.write_a11y_info(&mut node);
            assert_eq!(node.role(), gpui::Role::Button);
            assert_eq!(node.label(), Some("Save"));
            assert_eq!(node.is_disabled(), disabled);
            assert!(gpui::Element::id(&element).is_some());
        }
        assert!(
            AccessibilityProps::default()
                .disabled(true)
                .has_platform_state()
        );
        let _ = gpui::div()
            .id("generic")
            .accessibility(AccessibilityProps::default());
    }

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
