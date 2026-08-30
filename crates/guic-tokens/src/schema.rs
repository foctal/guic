use crate::Theme;
use schemars::Schema;

/// The JSON schema used to validate GUIC themes.
pub type ThemeSchema = Schema;

/// Builds the JSON schema for [`Theme`].
pub fn theme_schema() -> ThemeSchema {
    schemars::schema_for!(Theme)
}
