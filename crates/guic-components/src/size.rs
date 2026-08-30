/// Shared component size variants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ComponentSize {
    /// Compact size.
    Small,
    /// Default size.
    #[default]
    Medium,
    /// Large size.
    Large,
}
