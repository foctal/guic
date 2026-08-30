use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Typography tokens used by GUIC components.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyTokens {
    /// Default sans-serif family.
    pub sans_family: String,
    /// Default monospace family.
    pub mono_family: String,
    /// Small text size.
    pub text_sm: f32,
    /// Medium text size.
    pub text_md: f32,
    /// Large text size.
    pub text_lg: f32,
    /// Compact line height.
    pub line_height_sm: f32,
    /// Default line height.
    pub line_height_md: f32,
    /// Relaxed line height.
    pub line_height_lg: f32,
    /// Regular weight.
    pub weight_regular: u16,
    /// Medium weight.
    pub weight_medium: u16,
    /// Bold weight.
    pub weight_bold: u16,
}
