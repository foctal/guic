use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Layer order tokens for overlays.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayerTokens {
    /// Base content layer.
    pub base: i32,
    /// Dropdown layer.
    pub dropdown: i32,
    /// Popover layer.
    pub popover: i32,
    /// Tooltip layer.
    pub tooltip: i32,
    /// Sheet layer.
    pub sheet: i32,
    /// Modal layer.
    pub modal: i32,
    /// Notification layer.
    pub notification: i32,
}
