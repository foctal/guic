use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Corner radius tokens.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadiusTokens {
    /// No rounding.
    pub none: f32,
    /// Small radius.
    pub sm: f32,
    /// Medium radius.
    pub md: f32,
    /// Large radius.
    pub lg: f32,
    /// Extra large radius.
    pub xl: f32,
    /// Fully rounded radius.
    pub full: f32,
}
