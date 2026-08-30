//! Core runtime infrastructure for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod accessibility;
mod app;
mod element_ext;
mod events;
mod focus;
mod keyboard;
mod measure;
mod overlay;
mod persistence;
mod platform;
mod root;
mod services;
mod state;
mod window_ext;

pub use accessibility::{AccessibilityElementExt, AccessibilityProps, Role};
pub use app::init;
pub use element_ext::ElementExt;
pub use events::{SelectionChange, ValueChange};
pub use focus::{FocusDirection, FocusManager, FocusScopeId};
pub use keyboard::{
    CommandRegistrationError, CommandRouter, CommandScope, CommandSpec, KeyboardNavigation,
};
pub use measure::{measure, measure_if, measurements_enabled};
pub use overlay::{
    CloseReason, ClosedOverlay, OverlayEntry, OverlayId, OverlayKind, OverlayManager,
    OverlayOptions, OverlayPriority, overlay_portal,
};
pub use persistence::{JsonStore, LoadSource, PersistenceError, Recovered};
pub use platform::{OsKind, PlatformCapabilities};
pub use root::Root;
pub use services::{
    ApplicationMenuService, CancellationToken, CredentialService, DeepLink, DeepLinkService,
    DroppedItem, FileDialogFilter, FileDialogRequest, FileDialogService, InstanceDisposition,
    NativeCapabilities, NativeCapability, NativeCommand, NativeMenuItem, NativeServices,
    NotificationRequest, NotificationService, NotificationUrgency, SecretValue, ServiceError,
    ServiceErrorKind, ServiceFuture, SingleInstanceService, TrayDefinition, TrayService,
    UpdateInfo, UpdateService,
};
pub use state::GlobalState;
pub use window_ext::WindowExt;
