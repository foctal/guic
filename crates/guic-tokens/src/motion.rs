use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported easing names for token-driven motion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EasingKind {
    /// A balanced default easing.
    Standard,
    /// A more pronounced easing for emphasized transitions.
    Emphasized,
}

/// Motion tokens for transitions and animations.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotionTokens {
    /// Fast duration in milliseconds.
    pub fast_ms: u16,
    /// Normal duration in milliseconds.
    pub normal_ms: u16,
    /// Slow duration in milliseconds.
    pub slow_ms: u16,
    /// Default easing name.
    pub easing_standard: EasingKind,
    /// Emphasized easing name.
    pub easing_emphasized: EasingKind,
}
