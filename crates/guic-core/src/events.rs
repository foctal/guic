//! Shared event helper types used across GUIC crates.

/// A value-change event payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueChange<T> {
    /// The previous value.
    pub previous: T,
    /// The next value.
    pub next: T,
}

impl<T> ValueChange<T> {
    /// Creates a new value-change event payload.
    #[must_use]
    pub fn new(previous: T, next: T) -> Self {
        Self { previous, next }
    }
}

/// A selection-change event payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionChange<T> {
    /// The previously selected value.
    pub previous: T,
    /// The newly selected value.
    pub next: T,
}

impl<T> SelectionChange<T> {
    /// Creates a new selection-change event payload.
    #[must_use]
    pub fn new(previous: T, next: T) -> Self {
        Self { previous, next }
    }
}
