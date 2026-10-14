//! Native executable resources for application build scripts.
//!
//! Call [`build`] from `build.rs`. Ordinary Cargo builds read `Guic.toml`;
//! cargo-guic supplies a resolved, hashed request and verifies its acknowledgement.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Windows executable branding, separate from installer artwork.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WindowsResources {
    /// ICO file linked into the selected executable.
    pub icon: Option<PathBuf>,
    /// Application manifest XML; omitted to use the standard-user, DPI-aware default.
    pub manifest: Option<PathBuf>,
    /// Use a manifest linked by a dependency, such as GPUI, instead of embedding one.
    #[serde(default)]
    pub use_dependency_manifest: bool,
    /// Version resource company name.
    pub company_name: Option<String>,
    /// Version resource copyright statement.
    pub copyright: Option<String>,
}

/// Versioned CLI-to-build-script request. Paths must be absolute.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Handshake version.
    pub schema_version: u32,
    /// Selected Cargo package name.
    pub package: String,
    /// Selected Cargo binary target.
    pub bin: String,
    /// Display name.
    pub product_name: String,
    /// Cargo application version.
    pub version: String,
    /// Windows inputs.
    pub windows: WindowsResources,
}

/// Integrates native resources, failing the build if compilation is unsuccessful.
///
/// Add `guic-build` to build dependencies and call `guic_build::build()?` in
/// `build.rs`. Without the CLI, `[build.windows-resources]` in `Guic.toml` is used.
/// A multi-binary application must configure `build.bin` for ordinary Cargo builds.
pub fn build() -> Result<()> {
    emit_identity()?;
    println!("cargo:rerun-if-env-changed=GUIC_BUILD_REQUEST");
    println!("cargo:rerun-if-env-changed=GUIC_BUILD_REQUEST_HASH");
    println!("cargo:rerun-if-changed=Guic.toml");
    let cli = std::env::var("GUIC_BUILD_REQUEST").ok();
    let request = if let Some(json) = &cli {
        Some(serde_json::from_str::<Request>(json).context("invalid GUIC build request")?)
    } else if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        ordinary_request()?
    } else {
        None
    };
    if let Some(request) = request {
        if std::env::var("CARGO_PKG_NAME").as_deref() != Ok(&request.package) {
            return Ok(());
        }
        ensure!(
            request.schema_version == 1,
            "unsupported GUIC build handshake"
        );
        for path in [&request.windows.icon, &request.windows.manifest]
            .into_iter()
            .flatten()
        {
            ensure!(
                path.is_absolute() && path.is_file(),
                "native resource input must be an absolute regular file"
            );
            println!("cargo:rerun-if-changed={}", path.display());
        }
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            ensure!(
                std::env::var("TARGET")? == std::env::var("HOST")?,
                "GUIC native resources require a native Windows build"
            );
            compile(&request)?;
        }
        if cli.is_some() {
            let hash =
                std::env::var("GUIC_BUILD_REQUEST_HASH").context("missing GUIC request hash")?;
            ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid GUIC request hash"
            );
            println!("cargo:rustc-env=GUIC_BUILD_REQUEST_HASH={hash}");
        }
    }
    Ok(())
}

fn emit_identity() -> Result<()> {
    for key in ["GUIC_APP_PACKAGE", "GUIC_APP_IDENTIFIER", "GUIC_APP_NAME"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let package = std::env::var("CARGO_PKG_NAME")?;
    let mut identity = None;
    if std::env::var("GUIC_APP_PACKAGE").as_deref() == Ok(&package) {
        identity = Some((
            std::env::var("GUIC_APP_IDENTIFIER")?,
            std::env::var("GUIC_APP_NAME")?,
        ));
    } else {
        let base = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").context("missing manifest directory")?,
        );
        let path = base.join("Guic.toml");
        if path.exists() {
            let mut value: toml::Value = toml::from_str(&std::fs::read_to_string(path)?)?;
            let os = std::env::var("CARGO_CFG_TARGET_OS")?;
            if let Some(overlay) = value.get("platform").and_then(|v| v.get(&os)).cloned() {
                merge(&mut value, overlay);
            }
            if let Some(app) = value.get("app")
                && let Some(id) = app.get("identifier").and_then(toml::Value::as_str)
            {
                identity = Some((
                    id.into(),
                    app.get("product-name")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(&package)
                        .into(),
                ));
            }
        }
    }
    if let Some((id, name)) = identity {
        ensure!(
            !id.chars().any(char::is_control) && !name.chars().any(char::is_control),
            "application identity contains control characters"
        );
        println!("cargo:rustc-env=GUIC_APP_IDENTIFIER={id}");
        println!("cargo:rustc-env=GUIC_APP_NAME={name}");
    }
    Ok(())
}

fn ordinary_request() -> Result<Option<Request>> {
    let base = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").context("missing manifest directory")?,
    );
    let path = base.join("Guic.toml");
    if !path.exists() {
        return Ok(None);
    }
    let mut value: toml::Value = toml::from_str(&std::fs::read_to_string(path)?)?;
    if let Some(overlay) = value
        .get("platform")
        .and_then(|p| p.get("windows"))
        .cloned()
    {
        merge(&mut value, overlay);
    }
    let Some(resources) = value.get("build").and_then(|b| b.get("windows-resources")) else {
        return Ok(None);
    };
    let mut windows: WindowsResources = resources.clone().try_into()?;
    for path in [&mut windows.icon, &mut windows.manifest]
        .into_iter()
        .flatten()
    {
        *path = base.join(&*path).canonicalize()?;
    }
    let name = std::env::var("CARGO_PKG_NAME")?;
    Ok(Some(Request {
        schema_version: 1,
        package: name.clone(),
        bin: value
            .get("build")
            .and_then(|b| b.get("bin"))
            .and_then(toml::Value::as_str)
            .unwrap_or(&name)
            .into(),
        product_name: value
            .get("app")
            .and_then(|a| a.get("product-name"))
            .and_then(toml::Value::as_str)
            .unwrap_or(&name)
            .into(),
        version: std::env::var("CARGO_PKG_VERSION")?,
        windows,
    }))
}

fn merge(base: &mut toml::Value, overlay: toml::Value) {
    if let (Some(base), Some(overlay)) = (base.as_table_mut(), overlay.as_table()) {
        for (key, value) in overlay {
            if let Some(existing) = base.get_mut(key) {
                merge(existing, value.clone());
            } else {
                base.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay;
    }
}

/// Locates a tool in the installed Windows SDK without changing the environment.
/// Returns `None` on other hosts or when the SDK does not provide the tool.
pub fn windows_sdk_tool(tool: &str) -> Option<PathBuf> {
    if !cfg!(windows) || Path::new(tool).components().count() != 1 {
        return None;
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => "x86",
    };
    let mut roots = Vec::new();
    if let Some(path) = std::env::var_os("WindowsSdkVerBinPath") {
        roots.push(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("WindowsSdkDir") {
        roots.push(PathBuf::from(path).join("bin"));
    }
    for key in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(path) = std::env::var_os(key) {
            roots.push(PathBuf::from(path).join("Windows Kits/10/bin"));
        }
    }
    roots.iter().find_map(|root| sdk_tool_in(root, arch, tool))
}

fn sdk_tool_in(root: &Path, arch: &str, tool: &str) -> Option<PathBuf> {
    let direct = root.join(arch).join(tool);
    if direct.is_file() {
        return Some(direct);
    }
    let mut versions: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let version = name
                .to_str()?
                .split('.')
                .map(str::parse::<u32>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .ok()?;
            Some((version, entry.path()))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions
        .into_iter()
        .map(|(_, path)| path.join(arch).join(tool))
        .find(|p| p.is_file())
}

/// Default manifest uses standard-user privileges, common controls and per-monitor DPI.
pub const DEFAULT_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
 <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges></security></trustInfo>
 <dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
 <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings><dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness><longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware></windowsSettings></application>
</assembly>
"#;

fn quoted(value: &str) -> Result<String> {
    ensure!(
        !value.chars().any(char::is_control),
        "resource values must not contain control characters"
    );
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Generates deterministic Windows RC source; also usable for build-time inspection.
pub fn resource_script(request: &Request, manifest: &Path) -> Result<String> {
    ensure!(
        !request.windows.use_dependency_manifest || request.windows.manifest.is_none(),
        "use-dependency-manifest cannot be combined with a custom manifest"
    );
    let numeric = request
        .version
        .split(['-', '+'])
        .next()
        .context("missing version")?
        .split('.')
        .map(str::parse::<u16>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        numeric.len() == 3,
        "Windows version requires three u16 components"
    );
    let version = format!("{},{},{},0", numeric[0], numeric[1], numeric[2]);
    let mut rc = String::from("#pragma code_page(65001)\n");
    if !request.windows.use_dependency_manifest {
        rc.push_str(&format!(
            "1 24 {}\n",
            quoted(manifest.to_str().context("manifest path must be UTF-8")?)?
        ));
    }
    if let Some(icon) = &request.windows.icon {
        rc.push_str(&format!(
            "1 ICON {}\n",
            quoted(icon.to_str().context("icon path must be UTF-8")?)?
        ));
    }
    rc.push_str(&format!("1 VERSIONINFO\nFILEVERSION {version}\nPRODUCTVERSION {version}\nFILEFLAGSMASK 0x3fL\nFILEFLAGS 0\nFILEOS 0x40004L\nFILETYPE 1\nFILESUBTYPE 0\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904b0\"\n  BEGIN\n"));
    for (key, value) in [
        ("InternalName", request.bin.as_str()),
        ("ProductName", request.product_name.as_str()),
        ("FileDescription", &request.product_name),
        ("FileVersion", &request.version),
        ("ProductVersion", &request.version),
        ("OriginalFilename", &format!("{}.exe", request.bin)),
        (
            "CompanyName",
            request.windows.company_name.as_deref().unwrap_or(""),
        ),
        (
            "LegalCopyright",
            request.windows.copyright.as_deref().unwrap_or(""),
        ),
    ] {
        let value = quoted(value)?;
        rc.push_str(&format!(
            "   VALUE \"{key}\", {}\\0\"\n",
            &value[..value.len() - 1]
        ));
    }
    rc.push_str("  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n");
    Ok(rc)
}

#[cfg(windows)]
fn compile(request: &Request) -> Result<()> {
    let output = PathBuf::from(std::env::var_os("OUT_DIR").context("missing OUT_DIR")?);
    let default_manifest = output.join("guic.manifest");
    std::fs::write(&default_manifest, DEFAULT_MANIFEST)?;
    let manifest = request
        .windows
        .manifest
        .as_deref()
        .unwrap_or(&default_manifest);
    let script = output.join("guic.rc");
    std::fs::write(&script, resource_script(request, manifest)?)?;
    embed_resource::compile_for(&script, [&request.bin], embed_resource::NONE)
        .manifest_required()
        .map_err(|e| anyhow::anyhow!("native resource compilation failed: {e:?}"))?;
    // Use the resource manifest (ours or a dependency's), not a linker-generated one.
    println!("cargo:rustc-link-arg-bin={}=/MANIFEST:NO", request.bin);
    Ok(())
}
#[cfg(not(windows))]
fn compile(_: &Request) -> Result<()> {
    anyhow::bail!("Windows resource compilation requires Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rc_versions_and_escaping() -> Result<()> {
        let mut request = Request {
            schema_version: 1,
            package: "test".into(),
            bin: "test".into(),
            product_name: "App \"Example\"".into(),
            version: "1.2.3-beta.1".into(),
            windows: WindowsResources::default(),
        };
        let text = resource_script(&request, Path::new("C:/path with spaces/app.manifest"))?;
        assert!(text.contains("FILEVERSION 1,2,3,0"));
        assert!(text.contains("1 24 "));
        assert!(text.contains("App \\\"Example\\\""));
        request.windows.use_dependency_manifest = true;
        let text = resource_script(&request, Path::new("manifest"))?;
        assert!(!text.contains("1 24 "));
        assert!(text.contains("FILEVERSION 1,2,3,0"));
        request.windows.manifest = Some("custom.manifest".into());
        assert!(resource_script(&request, Path::new("manifest")).is_err());
        request.windows.manifest = None;
        request.version = "65536.0.0".into();
        assert!(resource_script(&request, Path::new("manifest")).is_err());
        assert!(quoted("bad\nvalue").is_err());
        Ok(())
    }
}
