//! Testable operating-system service boundaries for native applications.

use std::{
    collections::BTreeSet,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A boxed asynchronous service operation.
pub type ServiceFuture<T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send + 'static>>;

/// Operating-system capabilities commonly needed by desktop applications.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeCapability {
    /// Opening one or more paths through a native picker.
    OpenDialog,
    /// Choosing a destination through a native picker.
    SaveDialog,
    /// Sending operating-system notifications.
    Notifications,
    /// Installing or updating the native application menu.
    ApplicationMenu,
    /// Installing or updating a tray or status item.
    TrayItem,
    /// Reading and writing platform-protected credentials.
    SecureCredentials,
    /// Receiving deep links.
    DeepLinks,
    /// Forwarding launches to an existing application instance.
    SingleInstance,
    /// Checking for and handing off application updates.
    Updates,
}

/// An immutable set of capabilities exposed by a platform adapter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeCapabilities(BTreeSet<NativeCapability>);

impl NativeCapabilities {
    /// Creates an empty capability set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a capability set from an iterator.
    #[must_use]
    pub fn from_capabilities(capabilities: impl IntoIterator<Item = NativeCapability>) -> Self {
        Self(capabilities.into_iter().collect())
    }

    /// Returns whether a capability is available.
    #[must_use]
    pub fn supports(&self, capability: NativeCapability) -> bool {
        self.0.contains(&capability)
    }

    /// Returns all capabilities in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = NativeCapability> + '_ {
        self.0.iter().copied()
    }
}

/// Stable classification for native service failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceErrorKind {
    /// The adapter does not implement the requested capability.
    Unsupported,
    /// The user or operating system denied permission.
    PermissionDenied,
    /// The user cancelled an interactive operation.
    Cancelled,
    /// A normally supported service is temporarily unavailable.
    Unavailable,
    /// The request failed validation.
    InvalidRequest,
    /// The platform operation failed.
    Platform,
}

/// A user-presentable native service failure with a stable programmatic kind.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct ServiceError {
    kind: ServiceErrorKind,
    message: String,
    retryable: bool,
}

impl ServiceError {
    /// Creates a service failure.
    #[must_use]
    pub fn new(kind: ServiceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable: false,
        }
    }

    /// Creates an unsupported-capability failure.
    #[must_use]
    pub fn unsupported(capability: NativeCapability) -> Self {
        Self::new(
            ServiceErrorKind::Unsupported,
            format!("native capability {capability:?} is not available"),
        )
    }

    /// Marks whether retrying the same operation may succeed.
    #[must_use]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Returns the stable failure classification.
    #[must_use]
    pub fn kind(&self) -> ServiceErrorKind {
        self.kind
    }

    /// Returns whether the application should offer a retry action.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }
}

/// Cooperative cancellation shared by application work and service adapters.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Creates a token in the active state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Calling this more than once is harmless.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Returns whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Returns a cancelled error when cancellation was requested.
    pub fn check(&self) -> Result<(), ServiceError> {
        if self.is_cancelled() {
            Err(ServiceError::new(
                ServiceErrorKind::Cancelled,
                "operation was cancelled",
            ))
        } else {
            Ok(())
        }
    }
}

/// A file-type filter for native open and save dialogs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileDialogFilter {
    /// User-facing filter label.
    pub label: String,
    /// Extensions without a leading dot.
    pub extensions: Vec<String>,
}

impl FileDialogFilter {
    /// Creates a normalized filter, discarding blank extensions.
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let extensions = extensions
            .into_iter()
            .map(Into::into)
            .map(|extension: String| extension.trim().trim_start_matches('.').to_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        Self {
            label: label.into(),
            extensions,
        }
    }
}

/// Configuration shared by native open and save dialogs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileDialogRequest {
    /// Optional dialog title.
    pub title: Option<String>,
    /// Optional initial directory.
    pub initial_directory: Option<PathBuf>,
    /// Optional filename proposed by a save dialog.
    pub suggested_name: Option<String>,
    /// Allowed file types.
    pub filters: Vec<FileDialogFilter>,
    /// Whether an open dialog may return several paths.
    pub multiple: bool,
}

/// Native file picker operations.
pub trait FileDialogService: Send + Sync {
    /// Opens a native path picker.
    fn open(&self, request: FileDialogRequest) -> ServiceFuture<Vec<PathBuf>>;

    /// Opens a native save destination picker.
    fn save(&self, request: FileDialogRequest) -> ServiceFuture<Option<PathBuf>>;
}

/// Urgency requested for an operating-system notification.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationUrgency {
    /// A low-priority informational notification.
    Low,
    /// Normal priority.
    #[default]
    Normal,
    /// An urgent notification that may use stronger platform presentation.
    Critical,
}

/// A request to display an operating-system notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationRequest {
    /// Stable identifier used to replace or withdraw a notification.
    pub id: String,
    /// Notification title.
    pub title: String,
    /// Notification body.
    pub body: String,
    /// Requested urgency.
    pub urgency: NotificationUrgency,
}

/// Operating-system notification operations.
pub trait NotificationService: Send + Sync {
    /// Returns whether notification permission is currently available.
    fn permission(&self) -> ServiceFuture<bool>;

    /// Requests notification permission when the platform supports prompting.
    fn request_permission(&self) -> ServiceFuture<bool>;

    /// Displays or replaces a notification.
    fn notify(&self, request: NotificationRequest) -> ServiceFuture<()>;

    /// Withdraws a previously displayed notification.
    fn withdraw(&self, id: String) -> ServiceFuture<()>;
}

/// An application command exposed through a native menu or tray item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCommand {
    /// Stable command identifier routed through the application command router.
    pub id: String,
    /// User-facing label.
    pub label: String,
    /// Whether the command can currently be invoked.
    pub enabled: bool,
    /// Whether the command represents a checked toggle.
    pub checked: Option<bool>,
}

/// A native application-menu entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeMenuItem {
    /// A command item.
    Command(NativeCommand),
    /// A visual separator.
    Separator,
    /// A nested submenu.
    Submenu {
        /// User-facing submenu label.
        label: String,
        /// Nested entries.
        items: Vec<NativeMenuItem>,
    },
}

/// Application-menu installation and command delivery.
pub trait ApplicationMenuService: Send + Sync {
    /// Replaces the complete application menu.
    fn set_menu(&self, menu: Vec<NativeMenuItem>) -> ServiceFuture<()>;

    /// Returns the next invoked command identifier, or `None` during shutdown.
    fn next_command(&self) -> ServiceFuture<Option<String>>;
}

/// Definition of an operating-system tray or status item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayDefinition {
    /// Stable tray item identifier.
    pub id: String,
    /// Accessible label and tooltip text.
    pub label: String,
    /// Optional encoded image bytes interpreted by the adapter.
    pub icon: Option<Vec<u8>>,
    /// Commands presented by the tray menu.
    pub menu: Vec<NativeMenuItem>,
}

/// Tray or status-item lifecycle and command delivery.
pub trait TrayService: Send + Sync {
    /// Creates or replaces a tray item.
    fn set_tray(&self, definition: TrayDefinition) -> ServiceFuture<()>;

    /// Removes a tray item. Removing an absent item must succeed.
    fn remove_tray(&self, id: String) -> ServiceFuture<()>;

    /// Returns the next invoked tray command, or `None` during shutdown.
    fn next_command(&self) -> ServiceFuture<Option<String>>;
}

/// Opaque credential bytes that deliberately omit `Debug` and `Display`.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretValue(Vec<u8>);

impl SecretValue {
    /// Wraps bytes received from a platform credential vault.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    /// Borrows the secret bytes.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Platform-protected credential storage.
pub trait CredentialService: Send + Sync {
    /// Reads a credential, returning `None` when it is absent.
    fn read(&self, service: String, account: String) -> ServiceFuture<Option<SecretValue>>;

    /// Creates or replaces a credential.
    fn write(&self, service: String, account: String, value: SecretValue) -> ServiceFuture<()>;

    /// Deletes a credential. Deleting an absent value must succeed.
    fn delete(&self, service: String, account: String) -> ServiceFuture<()>;
}

/// A validated deep link split into its routing components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeepLink {
    /// Lowercase URI scheme.
    pub scheme: String,
    /// Remaining URI text after `://`.
    pub target: String,
}

impl DeepLink {
    /// Parses a deep link and rejects ambiguous or unsafe schemes.
    pub fn parse(value: &str) -> Result<Self, ServiceError> {
        let (scheme, target) = value.split_once("://").ok_or_else(|| {
            ServiceError::new(ServiceErrorKind::InvalidRequest, "deep link has no scheme")
        })?;
        let valid_scheme = !scheme.is_empty()
            && scheme.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphabetic()
                    || (index > 0 && matches!(byte, b'0'..=b'9' | b'+' | b'-' | b'.'))
            });
        if !valid_scheme || target.is_empty() || target.chars().any(char::is_control) {
            return Err(ServiceError::new(
                ServiceErrorKind::InvalidRequest,
                "deep link is not valid",
            ));
        }
        Ok(Self {
            scheme: scheme.to_ascii_lowercase(),
            target: target.to_owned(),
        })
    }
}

/// Registration and delivery of validated deep links.
pub trait DeepLinkService: Send + Sync {
    /// Registers a URI scheme with the platform adapter.
    fn register_scheme(&self, scheme: String) -> ServiceFuture<()>;

    /// Returns the next link, or `None` during shutdown.
    fn next_link(&self) -> ServiceFuture<Option<DeepLink>>;
}

/// A normalized item received through an operating-system drag/drop boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DroppedItem {
    /// A local filesystem path.
    Path(PathBuf),
    /// A validated non-file URI.
    Link(DeepLink),
}

/// Result of acquiring a single-instance application lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceDisposition {
    /// This process owns the application instance.
    Primary,
    /// The launch was forwarded to an existing process and this process exits.
    Forwarded,
}

/// Single-instance launch routing.
pub trait SingleInstanceService: Send + Sync {
    /// Acquires the primary instance or forwards arguments to it.
    fn acquire_or_forward(&self, arguments: Vec<String>) -> ServiceFuture<InstanceDisposition>;
}

/// Metadata returned by an updater check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateInfo {
    /// Version offered by the updater.
    pub version: String,
    /// Human-readable release notes.
    pub notes: Option<String>,
}

/// Update checking and installer handoff.
pub trait UpdateService: Send + Sync {
    /// Checks for a newer application version.
    fn check(&self, cancellation: CancellationToken) -> ServiceFuture<Option<UpdateInfo>>;

    /// Downloads and hands the update to the platform installer.
    fn install(&self, update: UpdateInfo, cancellation: CancellationToken) -> ServiceFuture<()>;
}

/// Collection of optional application-owned native service adapters.
#[derive(Default)]
pub struct NativeServices {
    /// Native file dialog adapter.
    pub file_dialogs: Option<Arc<dyn FileDialogService>>,
    /// Operating-system notification adapter.
    pub notifications: Option<Arc<dyn NotificationService>>,
    /// Native application-menu adapter.
    pub application_menu: Option<Arc<dyn ApplicationMenuService>>,
    /// Tray or status-item adapter.
    pub tray: Option<Arc<dyn TrayService>>,
    /// Platform credential-vault adapter.
    pub credentials: Option<Arc<dyn CredentialService>>,
    /// Deep-link registration and delivery adapter.
    pub deep_links: Option<Arc<dyn DeepLinkService>>,
    /// Single-instance adapter.
    pub single_instance: Option<Arc<dyn SingleInstanceService>>,
    /// Application updater adapter.
    pub updates: Option<Arc<dyn UpdateService>>,
}

impl NativeServices {
    /// Reports the capabilities backed by installed adapters.
    #[must_use]
    pub fn capabilities(&self) -> NativeCapabilities {
        let mut capabilities = Vec::new();
        if self.file_dialogs.is_some() {
            capabilities.extend([NativeCapability::OpenDialog, NativeCapability::SaveDialog]);
        }
        if self.notifications.is_some() {
            capabilities.push(NativeCapability::Notifications);
        }
        if self.application_menu.is_some() {
            capabilities.push(NativeCapability::ApplicationMenu);
        }
        if self.tray.is_some() {
            capabilities.push(NativeCapability::TrayItem);
        }
        if self.credentials.is_some() {
            capabilities.push(NativeCapability::SecureCredentials);
        }
        if self.deep_links.is_some() {
            capabilities.push(NativeCapability::DeepLinks);
        }
        if self.single_instance.is_some() {
            capabilities.push(NativeCapability::SingleInstance);
        }
        if self.updates.is_some() {
            capabilities.push(NativeCapability::Updates);
        }
        NativeCapabilities::from_capabilities(capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_shared_and_idempotent() {
        let token = CancellationToken::new();
        let observer = token.clone();
        assert!(observer.check().is_ok());
        token.cancel();
        token.cancel();
        let error = observer.check().expect_err("cancelled token should fail");
        assert_eq!(error.kind(), ServiceErrorKind::Cancelled);
    }

    #[test]
    fn deep_links_validate_and_normalize_the_scheme() {
        let link = DeepLink::parse("GUIC+APP://open/project?id=7").expect("valid deep link");
        assert_eq!(link.scheme, "guic+app");
        assert_eq!(link.target, "open/project?id=7");
        assert!(DeepLink::parse("1invalid://target").is_err());
        assert!(DeepLink::parse("guic://").is_err());
        assert!(DeepLink::parse("guic://bad\nvalue").is_err());
    }

    #[test]
    fn filters_normalize_extensions() {
        let filter = FileDialogFilter::new("Images", [".PNG", " jpg ", ""]);
        assert_eq!(filter.extensions, ["png", "jpg"]);
    }

    #[test]
    fn installed_services_define_capabilities() {
        struct Dialogs;
        impl FileDialogService for Dialogs {
            fn open(&self, _: FileDialogRequest) -> ServiceFuture<Vec<PathBuf>> {
                Box::pin(async { Ok(Vec::new()) })
            }

            fn save(&self, _: FileDialogRequest) -> ServiceFuture<Option<PathBuf>> {
                Box::pin(async { Ok(None) })
            }
        }

        let services = NativeServices {
            file_dialogs: Some(Arc::new(Dialogs)),
            ..NativeServices::default()
        };
        let capabilities = services.capabilities();
        assert!(capabilities.supports(NativeCapability::OpenDialog));
        assert!(capabilities.supports(NativeCapability::SaveDialog));
        assert!(!capabilities.supports(NativeCapability::Notifications));
    }

    #[test]
    fn secret_debug_output_cannot_be_requested() {
        let secret = SecretValue::new(b"token".to_vec());
        assert_eq!(secret.expose(), b"token");
    }
}
