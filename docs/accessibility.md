# Accessibility

GUIC components must expose the same semantic information that a native control
would expose: role, accessible name, interactive state, keyboard reachability,
focus behavior, and token-driven contrast.

## Platform Mapping

`guic-core` provides `AccessibilityProps`, `Role`, and
`AccessibilityElementExt`. Components apply these props to GPUI stateful
elements so platform accessibility nodes receive:

- role mappings through AccessKit roles
- accessible labels through `aria_label`
- selected state through `aria_selected`
- expanded state through `aria_expanded`
- checked/toggled state through `aria_toggled`

GPUI currently maps GUIC roles, labels, descriptions, selected, expanded,
checked/toggled, and numeric range state. `disabled` remains in
`AccessibilityProps` so components can keep complete metadata and map it once
the platform surface exposes the corresponding setter.

## Component Requirements

Every interactive component must satisfy these requirements before it is marked
implemented:

- It has a semantic role or a documented reason for being decorative.
- It has an accessible name from visible text, a label builder, or stable host
  metadata.
- Its selected, expanded, checked, or toggled state is reflected when relevant.
- Pointer-only behavior has a keyboard path when the control can receive focus.
- Disabled controls do not emit actions and visually communicate the disabled
  state.
- Text and state indicators use theme tokens with adequate contrast in light,
  dark, and high-contrast themes.

## Review Gates

Accessibility review is required for these component groups:

- Overlays and dialogs: role, focus trap behavior, dismiss intent, and readable
  title/description.
- Menus and menubars: menu/menu-item roles, keyboard close behavior, disabled
  rows, and stable activation labels.
- Tables and trees: table/tree roles, row or item names, selection state, and
  expansion state.
- Forms: labels, keyboard activation, current value/state, and disabled
  behavior.

Automated tests should cover role mapping, state metadata, keyboard activation,
and host callback behavior where GPUI test support can observe it. Manual
review should verify screen-reader output and high-contrast rendering on target
platforms.
