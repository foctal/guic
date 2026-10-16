//! Read-only installed resources for the `guic-native-v1` packaging contract.
//!
//! The contract matches cargo-packager 0.11.8: macOS `Contents/Resources`,
//! Windows beside the executable, and Linux `../lib/<binary>` relative to
//! `usr/bin`. The Linux layout works in both deb installations and mounted or
//! extracted AppImages; no mount environment variables or working directory are used.

use std::{
    io,
    path::{Component, Path, PathBuf},
};

/// Read-only application resource directory, independent of the working directory.
#[derive(Clone, Debug)]
pub struct ResourceRoot(PathBuf);

impl ResourceRoot {
    /// Resolves installed resources, or an explicit absolute `GUIC_RESOURCE_DIR`.
    ///
    /// cargo-guic dev supplies that variable. For ordinary Cargo development,
    /// set it to a prepared resource tree or use [`Self::from_directory`].
    /// `binary` is the Cargo binary target name, without `.exe`.
    pub fn discover(binary: &str) -> io::Result<Self> {
        if let Some(root) = std::env::var_os("GUIC_RESOURCE_DIR") {
            return Self::from_directory(root);
        }
        Self::from_executable(&std::env::current_exe()?, std::env::consts::OS, binary)
    }

    /// Resolves the versioned layout from an explicit executable location.
    ///
    /// Useful for launchers and for validating a staged installation. `os` must
    /// be `macos`, `windows`, or `linux`. No source-tree fallback is attempted.
    pub fn from_executable(executable: &Path, os: &str, binary: &str) -> io::Result<Self> {
        logical_path(Path::new(binary))?;
        if binary.contains(['/', '\\']) || !executable.is_absolute() {
            return Err(invalid(
                "binary must be one filename and executable must be absolute",
            ));
        }
        let parent = executable
            .parent()
            .ok_or_else(|| invalid("executable has no parent"))?;
        let root = match os {
            "macos"
                if parent.file_name().is_some_and(|n| n == "MacOS")
                    && parent
                        .parent()
                        .and_then(Path::file_name)
                        .is_some_and(|n| n == "Contents") =>
            {
                parent.join("../Resources")
            }
            "windows" => parent.to_owned(),
            "linux" if parent.file_name().is_some_and(|n| n == "bin") => {
                parent.join("../lib").join(binary)
            }
            _ => {
                return Err(invalid(
                    "executable is outside the supported installed resource layout; use GUIC_RESOURCE_DIR for development",
                ));
            }
        };
        Self::from_directory(root)
    }

    /// Uses an existing absolute resource directory explicitly.
    pub fn from_directory(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(invalid("resource directory must be absolute"));
        }
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(invalid("resource root is not a directory"));
        }
        Ok(Self(root))
    }

    /// Returns the canonical root directory.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Resolves an existing logical path while rejecting traversal and escaping symlinks.
    pub fn resolve(&self, logical: impl AsRef<Path>) -> io::Result<PathBuf> {
        let logical = logical.as_ref();
        logical_path(logical)?;
        let resolved = self.0.join(logical).canonicalize()?;
        if !resolved.starts_with(&self.0) {
            return Err(invalid("resource resolves outside its root"));
        }
        Ok(resolved)
    }

    /// Reads an existing resource without exposing a writable storage abstraction.
    pub fn read(&self, logical: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(self.resolve(logical)?)
    }
}

fn logical_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || path.to_string_lossy().contains(['\\', ':'])
    {
        return Err(invalid(
            "resource path must contain only relative normal components",
        ));
    }
    Ok(())
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_layouts_share_one_logical_path() -> io::Result<()> {
        let temp = tempfile::tempdir()?;
        let base = temp.path().canonicalize()?;
        for (os, exe, resources) in [
            (
                "macos",
                "App.app/Contents/MacOS/app",
                "App.app/Contents/Resources",
            ),
            ("windows", "install/app.exe", "install"),
            ("linux", "deb/usr/bin/app", "deb/usr/lib/app"),
            ("linux", "AppDir/usr/bin/app", "AppDir/usr/lib/app"),
        ] {
            std::fs::create_dir_all(base.join(exe).parent().expect("fixture parent"))?;
            let root = base.join(resources);
            std::fs::create_dir_all(root.join("data"))?;
            std::fs::write(root.join("data/example.txt"), "fixture")?;
            let resolved = ResourceRoot::from_executable(&base.join(exe), os, "app")?;
            assert_eq!(resolved.read("data/example.txt")?, b"fixture");
            assert!(resolved.resolve("../example.txt").is_err());
            assert!(resolved.resolve("C:\\example.txt").is_err());
        }
        assert!(ResourceRoot::from_directory("relative").is_err());
        assert!(
            ResourceRoot::from_executable(&base.join("target/debug/app"), "linux", "app").is_err()
        );
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn escaping_symlink_is_not_a_resource() -> io::Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        std::fs::create_dir(root.join("assets"))?;
        std::fs::write(root.join("private"), "private")?;
        std::os::unix::fs::symlink("../private", root.join("assets/link"))?;
        assert!(
            ResourceRoot::from_directory(root.join("assets"))?
                .read("link")
                .is_err()
        );
        Ok(())
    }
}
