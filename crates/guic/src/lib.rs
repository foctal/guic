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

/// The matching GPUI runtime, re-exported for single-dependency applications.
/// Import `guic::gpui` as `gpui` when using GPUI derives or action macros.
pub use gpui;

/// Creates an application using the native backend for the current OS.
///
/// The default `application` feature enables this helper. Call [`open_window`]
/// in the application's run callback to initialize GUIC and mount its root.
#[cfg(all(
    feature = "application",
    any(target_os = "macos", target_os = "linux", target_os = "windows")
))]
pub fn application() -> gpui::Application {
    gpui_platform::application()
}

/// Initializes GUIC and opens a window with theme, overlays, and focus handling.
///
/// Existing applications can continue to construct [`core::Root`] themselves.
/// Keep a clone of the content entity in the builder if it is needed later.
pub fn open_window<V: gpui::Render>(
    cx: &mut gpui::App,
    options: gpui::WindowOptions,
    build: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> gpui::Entity<V>,
) -> anyhow::Result<gpui::WindowHandle<core::Root>> {
    use gpui::AppContext as _;
    init(cx);
    cx.open_window(options, |window, cx| {
        let view = build(window, cx);
        cx.new(|cx| core::Root::new(view, window, cx))
    })
}

/// Signed in-app updates for macOS app bundles, Windows NSIS, and Linux AppImage.
///
/// Enable the `updater` feature. Run blocking network and installation operations
/// on a background thread, outside the GUI event loop. Configure HTTPS endpoints
/// and the public key used by `cargo guic updater prepare`.
#[cfg(feature = "updater")]
pub use cargo_packager_updater as updater;

pub use guic_core as core;
pub use guic_macros::{shared_format, shared_string};
/// Desktop application identity, activation, lifecycle, menus, trays, and sidecars.
#[cfg(feature = "native")]
pub use guic_native as native;
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
