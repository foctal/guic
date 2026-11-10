# Accessibility

GUIC uses GPUI's AccessKit integration to expose control roles, labels, and
state. Keyboard behavior is described on each [component page](components.md).

## Metadata

`guic_core::AccessibilityProps` and `AccessibilityElementExt` map roles, labels,
descriptions, selection, expansion, checked state, and numeric values/ranges to
GPUI accessibility nodes. Use `.accessible(props)` to include disabled semantics
without adding a layout or focus node. The original `.accessibility(props)` method
preserves its concrete return type but does not forward `disabled`. Components
must also enforce disabled interaction in their event handlers.

For application controls:

- Give icon-only controls an accessible label with `.label(...)` or
  `.accessible_label(...)`, as supported by the component.
- Use stable IDs for repeated controls and retain application-owned focus handles.
- Keep controlled selection, expansion, and checked state in sync with callbacks.
- Preserve keyboard access when replacing a component's content or actions.
- Review text and focus contrast when supplying custom themes.

For custom components, apply `AccessibilityProps` through
`AccessibilityElementExt` to the stateful element representing the control.
Use visible text as its accessible name when possible.

Test keyboard navigation, focus restoration, and screen-reader output on each
target platform. Metadata alone does not establish screen-reader compatibility.
See [focus behavior](component-behavior.md#focus-lifecycle),
[theming](theming.md), and the [platform checklist](platform-smoke.md).
