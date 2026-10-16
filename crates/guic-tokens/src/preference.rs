use crate::{Theme, ThemeContextExt};
use gpui::{App, Global, Subscription, Window, WindowAppearance};
use serde::{Deserialize, Serialize};

/// User preference, kept separate from the resolved light/dark theme mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreference {
    /// Follow the operating system light or dark appearance.
    #[default]
    System,
    /// Always use the configured light theme.
    Light,
    /// Always use the configured dark theme.
    Dark,
}

/// Application-wide theme selection with configurable system appearance mappings.
///
/// Install the controller, then retain an [`observe`](Self::observe) subscription
/// for each window. Explicit light/dark preferences do not change on OS events.
#[derive(Clone, Debug)]
pub struct ThemeController {
    preference: ThemePreference,
    light: Theme,
    dark: Theme,
    high_contrast_light: Theme,
    high_contrast_dark: Theme,
    high_contrast: bool,
}

impl Global for ThemeController {}

impl Default for ThemeController {
    fn default() -> Self {
        Self {
            preference: ThemePreference::System,
            light: Theme::light(),
            dark: Theme::dark(),
            high_contrast_light: Theme::high_contrast_light(),
            high_contrast_dark: Theme::high_contrast_dark(),
            high_contrast: false,
        }
    }
}

impl ThemeController {
    /// Sets the initial preference, which can be restored from application settings.
    pub fn preference(mut self, preference: ThemePreference) -> Self {
        self.preference = preference;
        self
    }
    /// Replaces normal light and dark themes.
    pub fn themes(mut self, light: Theme, dark: Theme) -> Self {
        self.light = light;
        self.dark = dark;
        self
    }
    /// Replaces the themes used when the explicit high-contrast option is enabled.
    pub fn high_contrast_themes(mut self, light: Theme, dark: Theme) -> Self {
        self.high_contrast_light = light;
        self.high_contrast_dark = dark;
        self
    }
    /// Enables high contrast explicitly; GPUI does not expose an OS contrast flag.
    pub fn high_contrast(mut self, enabled: bool) -> Self {
        self.high_contrast = enabled;
        self
    }
    /// Installs the controller without changing unrelated application globals.
    pub fn install(self, cx: &mut App) {
        cx.set_global(self);
    }
    /// Returns the user's preference rather than its resolved appearance.
    pub fn selected(&self) -> ThemePreference {
        self.preference
    }
    /// Resolves an OS appearance to one of the configured themes.
    pub fn resolve(&self, appearance: WindowAppearance) -> &Theme {
        let light = if self.high_contrast {
            &self.high_contrast_light
        } else {
            &self.light
        };
        let dark = if self.high_contrast {
            &self.high_contrast_dark
        } else {
            &self.dark
        };
        match self.preference {
            ThemePreference::Light => light,
            ThemePreference::Dark => dark,
            ThemePreference::System => match appearance {
                WindowAppearance::Light | WindowAppearance::VibrantLight => light,
                WindowAppearance::Dark | WindowAppearance::VibrantDark => dark,
            },
        }
    }
    /// Applies a preference immediately and retains it for future appearance events.
    pub fn set_preference(preference: ThemePreference, window: &Window, cx: &mut App) {
        if !cx.has_global::<Self>() {
            Self::default().install(cx);
        }
        cx.global_mut::<Self>().preference = preference;
        Self::apply(window, cx);
    }
    /// Applies the initial appearance and observes future changes for this window.
    pub fn observe(window: &Window, cx: &mut App) -> Subscription {
        if !cx.has_global::<Self>() {
            Self::default().install(cx);
        }
        Self::apply(window, cx);
        window.observe_window_appearance(|window, cx| Self::apply(window, cx))
    }
    fn apply(window: &Window, cx: &mut App) {
        let theme = cx.global::<Self>().resolve(window.appearance()).clone();
        cx.set_theme(theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_preference_ignores_system_changes() {
        let controller = ThemeController::default();
        assert_eq!(
            controller.resolve(WindowAppearance::Light).mode,
            crate::ThemeMode::Light
        );
        assert_eq!(
            controller.resolve(WindowAppearance::Dark).mode,
            crate::ThemeMode::Dark
        );
        let controller = controller.preference(ThemePreference::Light);
        assert_eq!(
            controller.resolve(WindowAppearance::Dark).mode,
            crate::ThemeMode::Light
        );
        assert_eq!(
            serde_json::to_string(&ThemePreference::System).expect("serialize preference"),
            "\"system\""
        );
    }
}
