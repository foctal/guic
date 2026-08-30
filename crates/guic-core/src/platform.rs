use gpui::Global;

/// The current operating system family.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OsKind {
    /// Apple macOS.
    Macos,
    /// Microsoft Windows.
    Windows,
    /// Linux or another Unix-like desktop target.
    Linux,
    /// Any other unsupported or unknown target.
    Unknown,
}

/// Platform capability metadata shared with components.
#[derive(Clone, Debug)]
pub struct PlatformCapabilities {
    /// The detected operating system family.
    pub os_kind: OsKind,
}

impl Global for PlatformCapabilities {}

impl PlatformCapabilities {
    /// Detects the current platform capabilities.
    #[must_use]
    pub fn current() -> Self {
        let os_kind = if cfg!(target_os = "macos") {
            OsKind::Macos
        } else if cfg!(target_os = "windows") {
            OsKind::Windows
        } else if cfg!(target_os = "linux") {
            OsKind::Linux
        } else {
            OsKind::Unknown
        };

        Self { os_kind }
    }
}
