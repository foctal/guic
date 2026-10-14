use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

/// Stable application identity shared by runtime services and packaging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIdentity {
    identifier: String,
    name: String,
    version: String,
}

impl AppIdentity {
    /// Creates an identity with a reverse-domain identifier and display name.
    pub fn new(
        identifier: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self> {
        let identifier = identifier.into();
        let name = name.into();
        let version = version.into();
        ensure!(
            identifier.len() <= 255
                && identifier.split('.').count() >= 2
                && identifier.split('.').all(|part| !part.is_empty()
                    && !part.starts_with('-')
                    && !part.ends_with('-')
                    && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')),
            "application identifier must be a reverse-domain name"
        );
        ensure!(
            !name.trim().is_empty() && !name.chars().any(char::is_control),
            "invalid application name"
        );
        ensure!(
            !version.is_empty() && !version.chars().any(char::is_control),
            "invalid application version"
        );
        Ok(Self {
            identifier,
            name,
            version,
        })
    }

    /// Returns the stable reverse-domain identifier.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
    /// Returns the user-visible application name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the application version.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Installs process-wide identity before windows or notifications are created.
    pub fn install(&self, cx: &mut gpui::App) {
        cx.set_app_identity(&self.identifier, &self.name);
    }
}

/// Writable application directories, distinct from read-only bundled resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    /// User configuration directory.
    pub config: PathBuf,
    /// User data directory (roaming on Windows).
    pub data: PathBuf,
    /// Machine-local persistent data directory.
    pub local_data: PathBuf,
    /// Disposable cache directory.
    pub cache: PathBuf,
    /// Diagnostic log directory.
    pub logs: PathBuf,
}

impl AppPaths {
    /// Resolves platform conventions without creating directories.
    pub fn discover(identity: &AppIdentity) -> Result<Self> {
        let resolve = |base: Option<PathBuf>| -> Result<PathBuf> {
            Ok(base
                .ok_or_else(|| anyhow::anyhow!("user directory is unavailable"))?
                .join(identity.identifier()))
        };
        let local_data = resolve(dirs::data_local_dir())?;
        let logs = if cfg!(target_os = "macos") {
            resolve(dirs::home_dir().map(|p| p.join("Library/Logs")))?
        } else {
            local_data.join("logs")
        };
        Ok(Self {
            config: resolve(dirs::config_dir())?,
            data: resolve(dirs::data_dir())?,
            // Windows uses LocalAppData for both data and cache roots. Keep
            // disposable files separate from persistent data and instance locks.
            cache: if cfg!(windows) {
                local_data.join("cache")
            } else {
                resolve(dirs::cache_dir())?
            },
            local_data,
            logs,
        })
    }

    /// Uses an absolute root for portable installations or isolated tests.
    pub fn under(root: impl AsRef<Path>, identity: &AppIdentity) -> Result<Self> {
        ensure!(
            root.as_ref().is_absolute(),
            "application directory root must be absolute"
        );
        let root = root.as_ref().join(identity.identifier());
        Ok(Self {
            config: root.join("config"),
            data: root.join("data"),
            local_data: root.join("local-data"),
            cache: root.join("cache"),
            logs: root.join("logs"),
        })
    }

    /// Creates all writable directories. Existing data is preserved.
    pub fn create(&self) -> std::io::Result<()> {
        for path in [
            &self.config,
            &self.data,
            &self.local_data,
            &self.cache,
            &self.logs,
        ] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_cache_does_not_contain_persistent_data() -> Result<()> {
        let identity = AppIdentity::new("org.example.paths", "Paths", "1")?;
        let paths = AppPaths::discover(&identity)?;
        assert_eq!(paths.cache, paths.local_data.join("cache"));
        for persistent in [&paths.config, &paths.data, &paths.local_data, &paths.logs] {
            assert!(!persistent.starts_with(&paths.cache));
        }
        Ok(())
    }
    #[test]
    fn identities_cannot_escape_directories() -> Result<()> {
        for id in ["../other", "com..app", "com/app", "com.\\app", "com.app\n"] {
            assert!(AppIdentity::new(id, "App", "1.0").is_err());
        }
        let identity = AppIdentity::new("org.example.app", "Application", "1.0")?;
        let dir = tempfile::tempdir()?;
        let paths = AppPaths::under(dir.path(), &identity)?;
        paths.create()?;
        std::fs::write(paths.data.join("state"), "preserved")?;
        paths.create()?;
        assert_eq!(
            std::fs::read_to_string(paths.data.join("state"))?,
            "preserved"
        );
        assert!(AppPaths::under("relative", &identity).is_err());
        Ok(())
    }
}
