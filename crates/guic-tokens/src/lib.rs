//! Design tokens and theme infrastructure for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod color;
mod elevation;
mod layer;
mod motion;
mod preference;
mod radius;
mod schema;
mod spacing;
mod theme;
mod typography;

pub use color::ColorTokens;
pub use elevation::ElevationTokens;
pub use layer::LayerTokens;
pub use motion::{EasingKind, MotionTokens};
pub use preference::{ThemeController, ThemePreference};
pub use radius::RadiusTokens;
pub use schema::{ThemeSchema, theme_schema};
pub use spacing::SpacingTokens;
pub use theme::{Theme, ThemeContextExt, ThemeError, ThemeMode, ThemeName, ThemeRegistry, init};
pub use typography::TypographyTokens;
