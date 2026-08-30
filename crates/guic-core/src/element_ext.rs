//! Small GUIC-oriented extensions for GPUI elements.

use gpui::{IntoElement, ParentElement as _, Styled as _, div};

/// Convenience extensions shared by GUIC elements.
pub trait ElementExt: IntoElement + Sized {
    /// Wraps the element in a disabled visual treatment.
    fn when_disabled(self, disabled: bool) -> impl IntoElement {
        if disabled {
            div().opacity(0.5).child(self)
        } else {
            div().child(self)
        }
    }
}

impl<T> ElementExt for T where T: IntoElement + Sized {}
