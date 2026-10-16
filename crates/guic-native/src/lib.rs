//! Opt-in native desktop integration for GUIC applications.
//!
//! Initialize the application identity before creating windows. Keep native
//! handles on the GPUI thread and retain the activation owner for the app lifetime.

#![warn(missing_docs)]

mod activation;
mod identity;
mod lifecycle;
mod window_state;

pub use activation::{
    ActivationOwner, ActivationRequest, Instance, SingleInstanceAdapter, activate_window,
};
pub use identity::{AppIdentity, AppPaths};
pub use lifecycle::{Lifecycle, ShutdownDecision, ShutdownReason, ShutdownState};
pub use window_state::{SavedWindow, SavedWindowMode, WindowStateStore};

mod menu;
pub use menu::{DesktopMenuHandle, DesktopMenus, TrayClick, TrayMouseButton, route_native_command};

/// Loads the calling application's identity emitted by `guic_build::build()`.
/// The version always comes from the application's Cargo package.
#[macro_export]
macro_rules! application_identity {
    () => {
        $crate::AppIdentity::new(
            env!("GUIC_APP_IDENTIFIER"),
            env!("GUIC_APP_NAME"),
            env!("CARGO_PKG_VERSION"),
        )
    };
}

mod sidecar;
pub use sidecar::{SidecarChild, SidecarCommand, SidecarProcesses, Sidecars};

mod desktop;
pub use desktop::DesktopState;

#[cfg(windows)]
mod windows_job;
