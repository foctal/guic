use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Elevation tokens for layered surfaces.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationTokens {
    /// Popover elevation value.
    pub popover: u16,
    /// Dropdown elevation value.
    pub dropdown: u16,
    /// Dialog elevation value.
    pub dialog: u16,
    /// Tooltip elevation value.
    pub tooltip: u16,
    /// Notification elevation value.
    pub notification: u16,
}
