# Theming

`guic-tokens` provides light, dark, and high-contrast themes. Themes support JSON
loading and validation with the [theme schema](theme.schema.json).

## System appearance

Install `ThemeController` before constructing `Root`. It resolves
`ThemePreference::{System, Light, Dark}` and tracks window appearance changes.
Use `ThemeController::set_preference` to change the preference; explicit Light
or Dark ignores subsequent OS appearance changes. Custom palettes can be
supplied through the controller builders. High contrast is an explicit
application setting.

`guic::native::DesktopState` persists the preference in the application config
directory. Applications without a controller can set a theme with `cx.set_theme`.

## Text contrast

Secondary text and input placeholders use the foreground at 72% opacity.
The built-in themes provide at least 4.5:1 contrast on their background, muted,
and secondary surfaces. Check custom themes against the surfaces where text
actually appears. Disabled controls use a separate opacity treatment.

Primary and danger buttons composite translucent accents over the theme
background and render the result as an opaque surface. They choose black or
white text by relative luminance for normal, hover, and pressed states.
