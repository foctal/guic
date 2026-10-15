use anyhow::{Result, ensure};
use std::path::PathBuf;

pub(crate) fn native_format(target: &str, format: &str) -> Result<()> {
    let os = crate::plan::target_os(target)?;
    let arch = target.split('-').next().unwrap_or("");
    let arch = if arch == "i686" { "x86" } else { arch };
    ensure!(
        os == std::env::consts::OS
            && (arch == std::env::consts::ARCH || (os == "macos" && arch == "universal")),
        "packaging requires the native target OS and architecture"
    );
    ensure!(
        match os {
            "macos" => ["app", "dmg"].contains(&format),
            "windows" => target.ends_with("-msvc") && ["nsis", "msi", "msix"].contains(&format),
            "linux" => ["deb", "appimage"].contains(&format),
            _ => false,
        },
        "unsupported native packaging format: {format}"
    );
    ensure!(
        format != "msi" || ["x86", "x86_64"].contains(&arch),
        "the pinned WiX backend supports MSI only on x86 and x86_64"
    );
    Ok(())
}

pub(crate) fn find(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|p| p.join(program))
                .find(|p| is_executable(p))
        })
        .or_else(|| guic_build::windows_sdk_tool(program))
        .or_else(|| {
            ["/usr/sbin", "/sbin"]
                .iter()
                .map(|p| PathBuf::from(p).join(program))
                .find(|p| is_executable(p))
        })
}

pub(crate) fn required_tools(
    os: &str,
    formats: &[String],
    signing: &crate::config::Signing,
) -> Vec<&'static str> {
    let mut tools = Vec::new();
    if os == "windows" && formats.iter().any(|f| f == "msix") {
        tools.push("makeappx.exe");
    }
    if os == "linux" && formats.iter().any(|f| f == "appimage") {
        // The pinned backend executes a script with an /usr/bin/env bash shebang.
        tools.push("bash");
    }
    if os == "macos" {
        if formats.iter().any(|f| f == "dmg") {
            tools.extend(["hdiutil", "osascript"]);
        }
        if signing.code_signing == crate::config::Policy::Required {
            tools.push("codesign");
        }
        if signing.notarization == crate::config::Policy::Required {
            tools.extend(["xcrun", "ditto"]);
        }
    }
    if os == "windows" && signing.code_signing == crate::config::Policy::Required {
        tools.push("signtool.exe");
    }
    tools
}

fn is_executable(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // Check executable access for this user, not just the file's existence.
        unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
    }
    #[cfg(not(unix))]
    true
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn tool_discovery_requires_executable_access() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("tool");
        std::fs::write(&path, "#!/bin/sh\n")?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        assert!(!is_executable(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        assert!(is_executable(&path));
        assert!(!is_executable(directory.path()));
        assert!(
            required_tools("linux", &["appimage".into()], &Default::default()).contains(&"bash")
        );
        assert!(!required_tools("linux", &["deb".into()], &Default::default()).contains(&"bash"));
        Ok(())
    }
}
