use anyhow::{Result, ensure};
use std::path::PathBuf;

pub(crate) fn native_format(target: &str, format: &str) -> Result<()> {
    let os = crate::plan::target_os(target)?;
    let arch = target.split('-').next().unwrap_or("");
    ensure!(
        os == std::env::consts::OS
            && (arch == std::env::consts::ARCH || (os == "macos" && arch == "universal")),
        "packaging requires the native target OS and architecture"
    );
    ensure!(
        match os {
            "macos" => ["app", "dmg"].contains(&format),
            "windows" => target.ends_with("-msvc") && format == "nsis",
            "linux" => ["deb", "appimage"].contains(&format),
            _ => false,
        },
        "unsupported native packaging format: {format}"
    );
    Ok(())
}

pub(crate) fn find(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|p| p.join(program))
                .find(|p| p.is_file())
        })
        .or_else(|| guic_build::windows_sdk_tool(program))
        .or_else(|| {
            ["/usr/sbin", "/sbin"]
                .iter()
                .map(|p| PathBuf::from(p).join(program))
                .find(|p| p.is_file())
        })
}

pub(crate) fn required_tools(
    os: &str,
    formats: &[String],
    signing: &crate::config::Signing,
) -> Vec<&'static str> {
    let mut tools = Vec::new();
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
