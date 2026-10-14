use super::{Output, Request, xml};
use crate::{config::Packager, platform, process, staging};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, path::Path, process::Command};

pub(super) fn validate(identifier: &str, version: &str, config: &Packager) -> Result<String> {
    ensure!(
        (3..=50).contains(&identifier.len())
            && identifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'),
        "MSIX identity must contain 3 to 50 ASCII letters, digits, periods or hyphens"
    );
    let publisher = config.msix_publisher.as_deref().context(
        "MSIX requires backend.cargo-packager.msix-publisher matching the certificate subject",
    )?;
    ensure!(
        !publisher.trim().is_empty(),
        "MSIX publisher must not be empty"
    );
    xml(publisher)?;
    let version = if let Some(version) = &config.msix_version {
        version.clone()
    } else {
        let version = semver::Version::parse(version)?;
        ensure!(
            version.pre.is_empty() && version.build.is_empty(),
            "MSIX prerelease/build versions require an explicit msix-version"
        );
        format!("{}.{}.{}.0", version.major, version.minor, version.patch)
    };
    let parts = version.split('.').collect::<Vec<_>>();
    ensure!(
        parts.len() == 4
            && parts.iter().all(|p| !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok()),
        "MSIX version must contain four integers between 0 and 65535"
    );
    Ok(version)
}

fn manifest(request: &Request) -> Result<String> {
    let version = validate(
        &request.identifier,
        &request.version,
        &request.customization,
    )?;
    let arch = match request.target.split('-').next() {
        Some("x86_64") => "x64",
        Some("i686") => "x86",
        Some("aarch64") => "arm64",
        _ => anyhow::bail!("unsupported MSIX architecture"),
    };
    let publisher = xml(request
        .customization
        .msix_publisher
        .as_deref()
        .context("MSIX publisher")?)?;
    let name = xml(&request.product_name)?;
    let display_publisher = xml(request
        .customization
        .publisher
        .as_deref()
        .unwrap_or(&request.product_name))?;
    let description = xml(request
        .description
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&request.product_name))?;
    let executable = xml(request
        .executable
        .file_name()
        .and_then(|s| s.to_str())
        .context("MSIX executable filename")?)?;
    let mut extensions = String::new();
    for protocol in &request.protocols {
        for scheme in &protocol.schemes {
            extensions.push_str(&format!(r#"<uap:Extension Category="windows.protocol"><uap:Protocol Name="{}" /></uap:Extension>"#, xml(scheme)?));
        }
    }
    for (index, association) in request.file_associations.iter().enumerate() {
        extensions.push_str(&format!(r#"<uap:Extension Category="windows.fileTypeAssociation"><uap:FileTypeAssociation Name="file{index}"><uap:SupportedFileTypes>"#));
        for extension in &association.extensions {
            extensions.push_str(&format!(
                "<uap:FileType>.{}</uap:FileType>",
                xml(extension.trim_start_matches('.'))?
            ));
        }
        extensions.push_str("</uap:SupportedFileTypes></uap:FileTypeAssociation></uap:Extension>");
    }
    if !extensions.is_empty() {
        extensions = format!("<Extensions>{extensions}</Extensions>");
    }
    Ok(format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
 xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
 xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
 IgnorableNamespaces="uap rescap">
 <Identity Name="{}" Version="{version}" Publisher="{publisher}" ProcessorArchitecture="{arch}" />
 <Properties><DisplayName>{name}</DisplayName><PublisherDisplayName>{display_publisher}</PublisherDisplayName><Logo>GuicAssets\StoreLogo.png</Logo></Properties>
 <Resources><Resource Language="en-us" /></Resources>
 <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.17763.0" /></Dependencies>
 <Applications><Application Id="App" Executable="{executable}" EntryPoint="Windows.FullTrustApplication">
  <uap:VisualElements DisplayName="{name}" Description="{description}" Square150x150Logo="GuicAssets\Square150x150Logo.png" Square44x44Logo="GuicAssets\Square44x44Logo.png" BackgroundColor="transparent" />
  {extensions}
 </Application></Applications>
 <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"#,
        request.identifier
    ))
}

fn stage(request: &Request, root: &Path) -> Result<()> {
    let manifest = manifest(request)?;
    let mut destinations = BTreeSet::new();
    for reserved in [
        "AppxManifest.xml",
        "AppxBlockMap.xml",
        "AppxSignature.p7x",
        "[Content_Types].xml",
        "AppxMetadata",
        "GuicAssets",
    ] {
        staging::reserve(&mut destinations, Path::new(reserved))?;
    }
    for executable in std::iter::once(&request.executable).chain(&request.sidecars) {
        let name = Path::new(executable.file_name().context("MSIX binary filename")?);
        staging::reserve(&mut destinations, name)?;
        crate::config::reject_symlinks(executable)?;
        std::fs::copy(executable, root.join(name))?;
    }
    for resource in &request.resources {
        staging::reserve(&mut destinations, &resource.destination)?;
    }
    crate::resources::stage(&request.resources, root)?;
    let icon = request
        .icons
        .iter()
        .find(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("png") || s.eq_ignore_ascii_case("ico"))
        })
        .context("MSIX requires a PNG or ICO bundle icon")?;
    let icon = image::open(icon).context("decode MSIX icon")?;
    ensure!(icon.width() == icon.height(), "MSIX icon must be square");
    std::fs::create_dir(root.join("GuicAssets"))?;
    for (file, size) in [
        ("StoreLogo.png", 50),
        ("Square44x44Logo.png", 44),
        ("Square150x150Logo.png", 150),
    ] {
        icon.resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .save(root.join("GuicAssets").join(file))?;
    }
    std::fs::write(root.join("AppxManifest.xml"), manifest)?;
    Ok(())
}

pub(super) fn package(request: &Request) -> Result<Vec<Output>> {
    let root = tempfile::Builder::new()
        .prefix("msix-payload-")
        .tempdir_in(&request.output_directory)?;
    stage(request, root.path())?;
    let executable = root.path().join(
        request
            .executable
            .file_name()
            .context("MSIX binary filename")?,
    );
    crate::signing::sidecars(
        &request.signing,
        "windows",
        request.execute_signing,
        &[executable],
        &mut crate::signing::Native,
    )?;
    let output = request
        .output_directory
        .join(format!("{}.msix", request.identifier));
    let tool = platform::find("makeappx.exe").context("MSIX requires Windows SDK makeappx.exe")?;
    // Keep SDK manifest and payload validation enabled; never pass /nv.
    process::status(
        Command::new(tool)
            .arg("pack")
            .arg("/d")
            .arg(root.path())
            .arg("/p")
            .arg(&output)
            .arg("/o"),
    )
    .context("MakeAppx package generation")?;
    crate::signing::sidecars(
        &request.signing,
        "windows",
        request.execute_signing,
        std::slice::from_ref(&output),
        &mut crate::signing::Native,
    )?;
    Ok(vec![Output {
        format: "msix".into(),
        paths: vec![output],
        verification: Default::default(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(root: &Path) -> Result<Request> {
        let executable = root.join("fixture.exe");
        std::fs::copy(std::env::current_exe()?, &executable)?;
        let icon = root.join("icon.png");
        image::RgbaImage::from_pixel(64, 64, image::Rgba([20, 80, 160, 255])).save(&icon)?;
        Ok(Request {
            schema_version: 1,
            target: "x86_64-pc-windows-msvc".into(),
            format: "msix".into(),
            identifier: "org.example.fixture".into(),
            product_name: "Fixture & Tools".into(),
            version: "1.2.3".into(),
            bin: "fixture".into(),
            executable,
            sidecars: vec![],
            output_directory: root.to_owned(),
            icons: vec![icon],
            deployment_target: None,
            description: None,
            authors: vec![],
            homepage: None,
            customization: Packager {
                msix_publisher: Some("CN=Example".into()),
                ..Default::default()
            },
            trial: true,
            execute_signing: false,
            signing: Default::default(),
            resources: vec![],
            protocols: vec![crate::config::Protocol {
                schemes: vec!["fixture".into()],
                name: None,
            }],
            file_associations: vec![crate::config::FileAssociation {
                extensions: vec!["fixture".into()],
                name: None,
                description: None,
                mime_type: None,
            }],
        })
    }

    #[test]
    fn identity_and_version_validation() -> Result<()> {
        let config = Packager {
            msix_publisher: Some("CN=Example".into()),
            ..Default::default()
        };
        assert_eq!(validate("org.example.app", "1.2.3", &config)?, "1.2.3.0");
        for version in ["1.2.3-beta.1", "1.2.3+meta", "65536.0.0"] {
            assert!(validate("org.example.app", version, &config).is_err());
        }
        assert!(validate("bad_name", "1.0.0", &config).is_err());
        assert!(validate("org.example.app", "1.0.0", &Packager::default()).is_err());
        let mut config = config;
        config.msix_version = Some("1.2.3.4".into());
        assert_eq!(
            validate("org.example.app", "1.2.3-beta", &config)?,
            "1.2.3.4"
        );
        for version in ["1.2.3", "1.2.3.65536", "1.2.3.-1"] {
            config.msix_version = Some(version.into());
            assert!(validate("org.example.app", "1.0.0", &config).is_err());
        }
        Ok(())
    }

    #[test]
    fn payload_escapes_xml_preserves_resources_and_rejects_collisions() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut request = request(temp.path())?;
        let root = tempfile::tempdir()?;
        let text = manifest(&request)?;
        assert!(text.contains("Fixture &amp; Tools"));
        assert!(text.contains("windows.protocol"));
        assert!(text.contains("<uap:FileType>.fixture</uap:FileType>"));
        request.resources.push(crate::resources::Input {
            source: request.icons[0].clone(),
            destination: "assets/data.png".into(),
            sha256: crate::report::hash_file(&request.icons[0])?,
        });
        stage(&request, root.path())?;
        assert!(root.path().join("fixture.exe").is_file());
        assert!(root.path().join("assets/data.png").is_file());
        assert_eq!(
            image::image_dimensions(root.path().join("GuicAssets/StoreLogo.png"))?,
            (50, 50)
        );
        for destination in [
            "appxmanifest.xml",
            "GuicAssets/custom.png",
            "fixture.exe",
            "../escape",
        ] {
            request.resources[0].destination = destination.into();
            assert!(stage(&request, tempfile::tempdir()?.path()).is_err());
        }
        assert!(xml("bad\0value").is_err());
        Ok(())
    }

    #[test]
    #[cfg(windows)]
    #[ignore = "requires WiX (downloaded by cargo-packager); creates an unsigned MSI without installing it"]
    fn wix_packages_msi() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut request = request(temp.path())?;
        request.format = "msi".into();
        let icon = temp.path().join("icon.ico");
        image::open(&request.icons[0])?.save(&icon)?;
        request.icons = vec![icon];
        let outputs = super::super::package_and_verify(&request)?;
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].format, "msi");
        assert_eq!(
            outputs[0].paths[0].extension().and_then(|s| s.to_str()),
            Some("msi")
        );
        assert!(outputs[0].paths[0].metadata()?.len() > 0);
        Ok(())
    }

    #[test]
    #[cfg(windows)]
    #[ignore = "requires the Windows SDK; creates and unpacks an unsigned MSIX without installing it"]
    fn sdk_packages_and_unpacks_payload() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut request = request(temp.path())?;
        let helper = temp.path().join("helper.exe");
        std::fs::copy(&request.executable, &helper)?;
        request.sidecars.push(helper);
        request.resources.push(crate::resources::Input {
            source: request.icons[0].clone(),
            destination: "assets/data.png".into(),
            sha256: crate::report::hash_file(&request.icons[0])?,
        });
        let output = super::super::package_and_verify(&request)?;
        let unpacked = temp.path().join("unpacked");
        process::status(
            Command::new(platform::find("makeappx.exe").context("Windows SDK")?)
                .arg("unpack")
                .arg("/p")
                .arg(&output[0].paths[0])
                .arg("/d")
                .arg(&unpacked),
        )?;
        assert!(unpacked.join("AppxManifest.xml").is_file());
        assert!(unpacked.join("helper.exe").is_file());
        assert_eq!(
            crate::report::hash_file(&unpacked.join("assets/data.png"))?,
            request.resources[0].sha256
        );
        assert_eq!(
            crate::report::hash_file(&unpacked.join("fixture.exe"))?,
            crate::report::hash_file(&request.executable)?
        );
        Ok(())
    }
}
