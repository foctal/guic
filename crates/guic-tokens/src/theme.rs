use crate::{
    ColorTokens, ElevationTokens, LayerTokens, MotionTokens, RadiusTokens, SpacingTokens,
    TypographyTokens,
};
use gpui::{App, Global, Hsla};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// The logical theme mode.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    /// Light mode.
    Light,
    /// Dark mode.
    Dark,
}

/// A stable theme name.
///
/// # Example
///
/// ```
/// use guic_tokens::ThemeName;
///
/// let name = ThemeName::new("  DefaultDark  ");
/// assert_eq!(name.as_str(), "DefaultDark");
/// ```
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ThemeName(String);

impl ThemeName {
    /// Creates a new normalized theme name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().trim().to_owned())
    }

    /// Returns the theme name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ThemeName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ThemeName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<ThemeName> for String {
    fn from(value: ThemeName) -> Self {
        value.0
    }
}

impl std::borrow::Borrow<str> for ThemeName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ThemeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A complete GUIC theme definition.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Stable theme name.
    pub name: ThemeName,
    /// Light or dark mode.
    pub mode: ThemeMode,
    /// Color tokens.
    pub color: ColorTokens,
    /// Spacing tokens.
    pub spacing: SpacingTokens,
    /// Radius tokens.
    pub radius: RadiusTokens,
    /// Typography tokens.
    pub typography: TypographyTokens,
    /// Elevation tokens.
    pub elevation: ElevationTokens,
    /// Motion tokens.
    pub motion: MotionTokens,
    /// Layer tokens.
    pub layer: LayerTokens,
}

/// Errors produced while loading or validating themes.
#[derive(Debug, Error)]
pub enum ThemeError {
    /// The supplied JSON string could not be parsed.
    #[error("failed to parse theme JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// A registry of installed themes.
///
/// # Example
///
/// ```no_run
/// use gpui::App;
/// use guic_tokens::{Theme, ThemeRegistry};
///
/// fn register_theme(cx: &mut App) {
///     guic_tokens::init(cx);
///     ThemeRegistry::global_mut(cx).register(Theme::light());
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct ThemeRegistry {
    themes: BTreeMap<ThemeName, Theme>,
}

impl Global for ThemeRegistry {}
impl Global for Theme {}

impl ThemeRegistry {
    /// Returns the registered theme registry.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the registered theme registry mutably.
    #[must_use]
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Registers a theme by name.
    pub fn register(&mut self, theme: Theme) {
        let replaced = self.themes.insert(theme.name.clone(), theme);
        if replaced.is_some() {
            tracing::warn!("replaced existing theme registration");
        }
    }

    /// Returns an iterator over the registered themes in sorted order.
    pub fn themes(&self) -> impl Iterator<Item = &Theme> {
        self.themes.values()
    }

    /// Returns a theme by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }
}

impl Theme {
    /// Built-in default dark theme name.
    pub const DEFAULT_DARK_NAME: &'static str = "DefaultDark";
    /// Built-in default light theme name.
    pub const DEFAULT_LIGHT_NAME: &'static str = "DefaultLight";
    /// Built-in high-contrast dark theme name.
    pub const HIGH_CONTRAST_DARK_NAME: &'static str = "HighContrastDark";
    /// Built-in high-contrast light theme name.
    pub const HIGH_CONTRAST_LIGHT_NAME: &'static str = "HighContrastLight";

    /// Returns the active global theme.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the active global theme mutably.
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Parses a theme from JSON.
    pub fn from_json_str(json: &str) -> Result<Self, ThemeError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Returns the built-in default dark theme.
    #[must_use]
    pub fn dark() -> Self {
        Self::from_json_str(include_str!("../themes/default-dark.json"))
            .expect("built-in dark theme JSON should be valid")
    }

    /// Returns the built-in default light theme.
    #[must_use]
    pub fn light() -> Self {
        Self::from_json_str(include_str!("../themes/default-light.json"))
            .expect("built-in light theme JSON should be valid")
    }

    /// Returns the built-in high-contrast dark theme.
    #[must_use]
    pub fn high_contrast_dark() -> Self {
        Self::from_json_str(include_str!("../themes/high-contrast-dark.json"))
            .expect("built-in high-contrast dark theme JSON should be valid")
    }

    /// Returns the built-in high-contrast light theme.
    #[must_use]
    pub fn high_contrast_light() -> Self {
        Self::from_json_str(include_str!("../themes/high-contrast-light.json"))
            .expect("built-in high-contrast light theme JSON should be valid")
    }

    /// Returns the background color as HSLA.
    #[must_use]
    pub fn background(&self) -> Hsla {
        self.color.background.into()
    }

    /// Returns the foreground color as HSLA.
    #[must_use]
    pub fn foreground(&self) -> Hsla {
        self.color.foreground.into()
    }

    /// Returns the muted color as HSLA.
    #[must_use]
    pub fn muted(&self) -> Hsla {
        self.color.muted.into()
    }

    /// Returns a muted foreground color for secondary text and placeholder text.
    #[must_use]
    pub fn muted_foreground(&self) -> Hsla {
        match self.mode {
            ThemeMode::Light => self.foreground().opacity(0.42),
            ThemeMode::Dark => self.foreground().opacity(0.48),
        }
    }

    /// Returns the border color as HSLA.
    #[must_use]
    pub fn border(&self) -> Hsla {
        self.color.border.into()
    }

    /// Returns the ring color as HSLA.
    #[must_use]
    pub fn ring(&self) -> Hsla {
        self.color.ring.into()
    }

    /// Returns the primary color as HSLA.
    #[must_use]
    pub fn primary(&self) -> Hsla {
        self.color.primary.into()
    }

    /// Returns the secondary color as HSLA.
    #[must_use]
    pub fn secondary(&self) -> Hsla {
        self.color.secondary.into()
    }

    /// Returns the accent color as HSLA.
    #[must_use]
    pub fn accent(&self) -> Hsla {
        self.color.accent.into()
    }

    /// Returns the success color as HSLA.
    #[must_use]
    pub fn success(&self) -> Hsla {
        self.color.success.into()
    }

    /// Returns the warning color as HSLA.
    #[must_use]
    pub fn warning(&self) -> Hsla {
        self.color.warning.into()
    }

    /// Returns the danger color as HSLA.
    #[must_use]
    pub fn danger(&self) -> Hsla {
        self.color.danger.into()
    }

    /// Returns the informational color as HSLA.
    #[must_use]
    pub fn info(&self) -> Hsla {
        self.color.info.into()
    }
}

/// Convenience methods for reading and updating the active theme.
pub trait ThemeContextExt {
    /// Returns the active theme.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui::App;
    /// use guic_tokens::{ThemeContextExt, Theme};
    ///
    /// fn read_theme(cx: &mut App) {
    ///     guic_tokens::init(cx);
    ///     let _: &Theme = cx.theme();
    /// }
    /// ```
    fn theme(&self) -> &Theme;

    /// Sets the active theme.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gpui::App;
    /// use guic_tokens::{Theme, ThemeContextExt};
    ///
    /// fn install_theme(cx: &mut App) {
    ///     guic_tokens::init(cx);
    ///     cx.set_theme(Theme::light());
    /// }
    /// ```
    fn set_theme(&mut self, theme: Theme);
}

impl ThemeContextExt for App {
    fn theme(&self) -> &Theme {
        Theme::global(self)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.set_global(theme);
        self.refresh_windows();
    }
}

/// Initializes theme globals and built-in themes.
pub fn init(cx: &mut App) {
    if !cx.has_global::<ThemeRegistry>() {
        let mut registry = ThemeRegistry::default();
        registry.register(Theme::dark());
        registry.register(Theme::light());
        registry.register(Theme::high_contrast_dark());
        registry.register(Theme::high_contrast_light());
        cx.set_global(registry);
    }

    if !cx.has_global::<Theme>() {
        cx.set_global(Theme::dark());
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;
    use crate::schema::theme_schema;

    #[test]
    fn parses_builtin_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.name.as_str(), Theme::DEFAULT_DARK_NAME);
    }

    #[test]
    fn parses_all_builtin_themes() {
        assert_eq!(Theme::light().name.as_str(), Theme::DEFAULT_LIGHT_NAME);
        assert_eq!(
            Theme::high_contrast_dark().name.as_str(),
            Theme::HIGH_CONTRAST_DARK_NAME
        );
        assert_eq!(
            Theme::high_contrast_light().name.as_str(),
            Theme::HIGH_CONTRAST_LIGHT_NAME
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let invalid = r##"{
            "name":"Broken",
            "mode":"dark",
            "color":{"background":"#000","foreground":"#fff","muted":"#111","border":"#222","ring":"#333","primary":"#444","secondary":"#555","accent":"#666","success":"#777","warning":"#888","danger":"#999","info":"#aaa"},
            "spacing":{"x0":0.0,"x0_5":2.0,"x1":4.0,"x1_5":6.0,"x2":8.0,"x3":12.0,"x4":16.0,"x5":20.0,"x6":24.0,"x8":32.0,"x10":40.0,"x12":48.0},
            "radius":{"none":0.0,"sm":2.0,"md":4.0,"lg":8.0,"xl":12.0,"full":999.0},
            "typography":{"sans_family":"Inter","mono_family":"JetBrains Mono","text_sm":12.0,"text_md":14.0,"text_lg":16.0,"line_height_sm":16.0,"line_height_md":20.0,"line_height_lg":24.0,"weight_regular":400,"weight_medium":500,"weight_bold":700},
            "elevation":{"popover":1,"dropdown":2,"dialog":3,"tooltip":4,"notification":5},
            "motion":{"fast_ms":120,"normal_ms":180,"slow_ms":240,"easing_standard":"standard","easing_emphasized":"emphasized"},
            "layer":{"base":0,"dropdown":10,"popover":20,"tooltip":30,"sheet":40,"modal":50,"notification":60},
            "unexpected":true
        }"##;

        assert!(Theme::from_json_str(invalid).is_err());
    }

    #[test]
    fn schema_generation_is_deterministic() {
        let left = serde_json::to_value(theme_schema()).expect("schema should serialize");
        let right = serde_json::to_value(theme_schema()).expect("schema should serialize");
        assert_eq!(left, right);
    }

    #[test]
    fn registry_lookup_returns_registered_theme() {
        let mut registry = super::ThemeRegistry::default();
        let theme = Theme::light();
        let name = theme.name.clone();

        registry.register(theme);

        assert_eq!(
            registry.get(name.as_str()).map(|theme| theme.name.as_str()),
            Some(name.as_str())
        );
    }
}
