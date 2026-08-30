# Component Guidelines

Components should use shared tokens, predictable builders, and explicit keyboard
and accessibility behavior.

## Accessibility

Interactive components must apply `AccessibilityProps` through
`AccessibilityElementExt` on the stateful GPUI element that represents the
control. Use visible text as the accessible label whenever possible; icon-only
controls must provide a label builder.

Components that manage selection, expansion, or checked state must expose that
state through platform accessibility metadata. Components that cannot expose a
field because GPUI lacks a setter must keep the metadata in their model and
document the limitation in `docs/accessibility.md`.
