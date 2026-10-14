# Theming

Themes are defined in `crates/guic-tokens`, support JSON loading, and can be
validated via the generated JSON schema.

## System appearance and persisted preference

`ThemeController` separates `ThemePreference::{System, Light, Dark}` from the
resolved theme mode. Install it before constructing `Root`; Root retains the
window appearance subscription and applies the initial appearance immediately.
Use `ThemeController::set_preference` to apply a change. Explicit Light or Dark
ignores later OS light/dark changes. Custom normal and high-contrast palettes
can be supplied through the controller builders. High contrast remains an
explicit application setting because GPUI 0.4.0 exposes no OS contrast flag.
`guic::native::DesktopState` loads and persists the preference in the application
config directory. Without a controller, existing manual `cx.set_theme` behavior
is unchanged.
