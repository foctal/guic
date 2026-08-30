use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Spacing tokens used by GUIC layouts and components.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpacingTokens {
    /// Spacing scale `0`.
    pub x0: f32,
    /// Spacing scale `0.5`.
    pub x0_5: f32,
    /// Spacing scale `1`.
    pub x1: f32,
    /// Spacing scale `1.5`.
    pub x1_5: f32,
    /// Spacing scale `2`.
    pub x2: f32,
    /// Spacing scale `3`.
    pub x3: f32,
    /// Spacing scale `4`.
    pub x4: f32,
    /// Spacing scale `5`.
    pub x5: f32,
    /// Spacing scale `6`.
    pub x6: f32,
    /// Spacing scale `8`.
    pub x8: f32,
    /// Spacing scale `10`.
    pub x10: f32,
    /// Spacing scale `12`.
    pub x12: f32,
}
