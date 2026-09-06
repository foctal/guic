# Changelog

All notable changes to this project will be documented in this file.

## 0.2.0 - Unreleased

- Upgrade the guic-gpui runtime family to 0.3.1.
- Unify single-line control sizing and stabilize component identity.
- Preserve wrapper geometry and improve Select layout, overlay focus restoration,
  and modal keyboard navigation.
- Add Dock customization, persistent split identities, resize constraints,
  protected pinned tabs, and host-mediated tab transfers.
- Improve terminal rendering, Unicode handling, search, shell integration, shared
  model access, and PTY lifecycle reporting.
- Add regression coverage and document component behavior and host contracts.

### Migration notes

- Direct `DockNode::Split` construction requires an `id`; older serialized
  layouts remain supported through `DockLayout::from_json`.
- Direct Dock drag payload construction must initialize `source_dock_id` and
  `source_window_id`. Hosts handle cross-Dock transfers explicitly.
- MultiSelect triggers stay on one line and clip excess selected chips.
- Review the [component behavior and host integration guide](docs/component-behavior.md)
  for control sizing, stable IDs, controlled focus lifecycles, and terminal ownership.

## 0.1.0 - 2026-08-30

- Initial public release.
