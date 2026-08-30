use gpui::Rgba;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Color tokens shared across GUIC components.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    /// Default surface background.
    pub background: Rgba,
    /// Default text color.
    pub foreground: Rgba,
    /// Muted surface color.
    pub muted: Rgba,
    /// Border color.
    pub border: Rgba,
    /// Focus ring color.
    pub ring: Rgba,
    /// Primary accent color.
    pub primary: Rgba,
    /// Secondary accent color.
    pub secondary: Rgba,
    /// Accent color for emphasis.
    pub accent: Rgba,
    /// Success color.
    pub success: Rgba,
    /// Warning color.
    pub warning: Rgba,
    /// Danger color.
    pub danger: Rgba,
    /// Informational color.
    pub info: Rgba,
}
