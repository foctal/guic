//! GUIC is a modern cross-platform native GUI toolkit for Rust.
//!
//! The project is under active development.
//!
//! # Example
//!
//! ```no_run
//! use gpui::{AppContext as _, ParentElement as _, div};
//! use guic::prelude::*;
//!
//! struct ExampleView;
//!
//! impl Render for ExampleView {
//!     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         div().child(format!("Theme: {}", cx.theme().name))
//!     }
//! }
//!
//! fn open_example(cx: &mut App) {
//!     guic::init(cx);
//!     let _window = cx.open_window(Default::default(), |window, cx| {
//!         cx.new(|cx| Root::new(cx.new(|_| ExampleView), window, cx))
//!     });
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod prelude;

/// Signed in-app updates for macOS app bundles, Windows NSIS, and Linux AppImage.
///
/// Enable the `updater` feature. Run blocking network and installation operations
/// on a background thread, outside the GUI event loop. Configure HTTPS endpoints
/// and the public key used by `cargo guic updater prepare`.
#[cfg(feature = "updater")]
pub use cargo_packager_updater as updater;

pub use guic_core as core;
pub use guic_macros::{shared_format, shared_string};
pub use guic_tokens as tokens;

/// Creates a theme name without requiring a direct `guic-tokens` dependency.
///
/// ```
/// let name = guic::theme_name!("Example");
/// assert_eq!(name.to_string(), "Example");
/// ```
#[macro_export]
macro_rules! theme_name {
    ($name:literal) => {
        $crate::tokens::ThemeName::from($name)
    };
}

#[cfg(feature = "assets")]
pub use guic_assets as assets;

#[cfg(feature = "components")]
pub use guic_components as components;

#[cfg(feature = "charts")]
pub use guic_charts as charts;

#[cfg(feature = "editor")]
pub use guic_editor as editor;

#[cfg(feature = "icons")]
pub use guic_icons as icons;

#[cfg(feature = "webview")]
pub use guic_webview as webview;

/// Initializes GUIC runtime systems.
///
/// This function is idempotent and safe to call more than once.
///
/// # Example
///
/// ```no_run
/// use gpui::App;
///
/// fn initialize(cx: &mut App) {
///     guic::init(cx);
///     guic::init(cx);
/// }
/// ```
pub fn init(cx: &mut gpui::App) {
    guic_core::init(cx);
    guic_tokens::init(cx);

    #[cfg(feature = "assets")]
    guic_assets::init(cx);

    #[cfg(feature = "components")]
    guic_components::init(cx);
}
