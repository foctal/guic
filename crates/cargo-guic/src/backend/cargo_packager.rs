use super::{Output, Request};
use anyhow::{Result, bail};
use cargo_packager::{
    Config, PackageFormat,
    config::{Binary, DebianConfig, DmgConfig, MacOsConfig, NsisConfig, Resource, WindowsConfig},
};

fn configuration(request: &Request) -> Result<Config> {
    let format = match request.format.as_str() {
        "app" => PackageFormat::App,
        "dmg" => PackageFormat::Dmg,
        "nsis" => PackageFormat::Nsis,
        "msi" => PackageFormat::Wix,
        "deb" => PackageFormat::Deb,
        "appimage" => PackageFormat::AppImage,
        _ => bail!("unsupported backend format"),
    };
    let mut config = Config::default();
    config.product_name = request.product_name.clone();
    config.version = request.version.clone();
    config.identifier = Some(request.identifier.clone());
    let mut executable = request.executable.clone();
    if request.target.contains("-windows-") {
        executable.set_extension("");
    }
    config.binaries = vec![Binary::new(executable).main(true)];
    for path in &request.sidecars {
        let mut path = path.clone();
        if request.target.contains("-windows-") {
            path.set_extension("");
        }
        config.binaries.push(Binary::new(path).main(false));
    }
    config.target_triple = Some(request.target.clone());
    config.out_dir = request.output_directory.clone();
    config.formats = Some(vec![format]);
    config.icons = Some(
        request
            .icons
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    );
    config.file_associations = Some(
        request
            .file_associations
            .iter()
            .map(|a| {
                let mut association =
                    cargo_packager::config::FileAssociation::new(a.extensions.clone());
                association.name = a.name.clone();
                association.description = a.description.clone();
                association.mime_type = a.mime_type.clone();
                association
            })
            .collect(),
    );
    config.deep_link_protocols = Some(
        request
            .protocols
            .iter()
            .map(|p| {
                let mut protocol = cargo_packager::config::DeepLinkProtocol::new(p.schemes.clone());
                protocol.name = p.name.clone();
                protocol
            })
            .collect(),
    );
    config.description = request.description.clone();
    config.authors = Some(request.authors.clone());
    config.homepage = request.homepage.clone();
    config.publisher = request.customization.publisher.clone();
    config.copyright = request.customization.copyright.clone();
    config.long_description = request.customization.long_description.clone();
    config.category = request
        .customization
        .category
        .as_ref()
        .map(|c| c.parse())
        .transpose()
        .map_err(|_| anyhow::anyhow!("invalid category"))?;
    config.license_file = request.customization.license_file.clone();
    config.resources = Some(
        request
            .resources
            .iter()
            .map(|r| Resource::Mapped {
                src: r.source.to_string_lossy().into_owned(),
                target: r.destination.clone(),
            })
            .collect(),
    );
    let mut dmg = DmgConfig::default();
    dmg.background = request.customization.dmg_background.clone();
    config.dmg = Some(dmg);
    let mut deb = DebianConfig::default().depends(request.customization.deb_depends.clone());
    deb.desktop_template = request.customization.deb_desktop_template.clone();
    config.deb = Some(deb);
    let mut nsis = NsisConfig::default();
    nsis.installer_icon = request.customization.nsis_installer_icon.clone();
    nsis.header_image = request.customization.nsis_header_image.clone();
    nsis.sidebar_image = request.customization.nsis_sidebar_image.clone();
    config.nsis = Some(nsis);
    if request.target.contains("-windows-")
        && request.signing.code_signing == crate::config::Policy::Required
    {
        let mut windows = WindowsConfig::default();
        windows.digest_algorithm = Some("sha256".into());
        windows.certificate_thumbprint = request.signing.windows_thumbprint.clone();
        windows.timestamp_url = request.signing.windows_timestamp_url.clone();
        windows.tsp = true;
        windows.allow_downgrades = false;
        config.windows = Some(windows);
    }
    if request.target.ends_with("-apple-darwin") {
        let mut macos = MacOsConfig::default();
        macos.minimum_system_version = request.deployment_target.clone();
        macos.info_plist_path = request.customization.macos_info_plist.clone();
        macos.entitlements = request
            .customization
            .macos_entitlements
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned());
        macos.frameworks = Some(
            request
                .customization
                .macos_frameworks
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        );
        if request.signing.code_signing == crate::config::Policy::Required {
            macos.signing_identity = request.signing.macos_identity.clone();
        }
        config.macos = Some(macos);
    }
    // No backend hooks, private credentials or automatic notarization are imported.
    if request.format == "msi" {
        // The pinned WiX template disables Handlebars escaping. These values
        // are XML attributes, not filesystem paths.
        config.product_name = super::xml(&request.product_name)?;
        config.publisher = Some(super::xml(&config.publisher())?);
        if let Some(associations) = &mut config.file_associations {
            for association in associations {
                association.name = association.name.as_deref().map(super::xml).transpose()?;
                association.description = association
                    .description
                    .as_deref()
                    .map(super::xml)
                    .transpose()?;
            }
        }
    }
    Ok(config)
}

pub(super) fn package(request: &Request) -> Result<Vec<Output>> {
    let mut config = configuration(request)?;
    // The pinned backend's default NSIS template evaluates protocol ownership
    // at compile time, leaving dead registrations behind after uninstall.
    // Retain our corrected template file until the backend finishes reading it.
    let _template = if request.format == "nsis" {
        use std::io::Write;
        let mut template = tempfile::NamedTempFile::new_in(&request.output_directory)?;
        template.write_all(include_bytes!("../../templates/installer.nsi"))?;
        template.flush()?;
        config.nsis.as_mut().expect("configured NSIS").template = Some(template.path().to_owned());
        Some(template)
    } else {
        None
    };
    Ok(cargo_packager::package(&config)?
        .into_iter()
        .map(|output| Output {
            format: if output.format == PackageFormat::Wix {
                "msi"
            } else {
                output.format.short_name()
            }
            .into(),
            paths: output.paths,
            verification: Default::default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nsis_protocol_cleanup_uses_runtime_ownership_guard() {
        let template = include_str!("../../templates/installer.nsi");
        let cleanup = template
            .split("; Delete deep links")
            .nth(1)
            .expect("protocol cleanup")
            .split("; Delete uninstaller")
            .next()
            .expect("cleanup boundary");
        assert!(cleanup.contains("ReadRegStr $R7"));
        assert!(cleanup.contains("${If} $R7 =="));
        assert!(cleanup.contains("${EndIf}"));
        assert!(!cleanup.contains("!if"));
        assert!(!cleanup.contains("!endif"));
        assert!(cleanup.find("${If}") < cleanup.find("DeleteRegKey"));
        assert!(cleanup.find("DeleteRegKey") < cleanup.find("${EndIf}"));
    }

    #[test]
    fn adapter_disables_hooks_and_signing() -> Result<()> {
        let request = Request {
            schema_version: 1,
            target: "aarch64-apple-darwin".into(),
            format: "app".into(),
            identifier: "com.example.fixture".into(),
            product_name: "Fixture".into(),
            version: "1.0.0".into(),
            bin: "fixture".into(),
            executable: "/stage/fixture".into(),
            sidecars: vec![],
            output_directory: "/output".into(),
            icons: Vec::new(),
            deployment_target: Some("12.0".into()),
            description: None,
            authors: Vec::new(),
            homepage: None,
            customization: Default::default(),
            trial: true,
            execute_signing: false,
            signing: Default::default(),
            resources: vec![],
            file_associations: vec![],
            protocols: vec![],
        };
        let config = configuration(&request)?;
        assert!(config.before_packaging_command.is_none());
        assert!(config.before_each_package_command.is_none());
        assert!(
            config.macos.as_ref().is_some_and(
                |m| m.signing_identity.is_none() && m.notarization_credentials.is_none()
            )
        );
        assert_eq!(config.binaries[0].path, request.executable);
        let mut request = request;
        request.target = "x86_64-pc-windows-msvc".into();
        request.format = "msi".into();
        request.executable = "/stage/fixture.exe".into();
        request.product_name = "Fixture & Tools".into();
        let config = configuration(&request)?;
        assert_eq!(config.formats, Some(vec![PackageFormat::Wix]));
        assert_eq!(config.product_name, "Fixture &amp; Tools");
        assert!(config.windows.is_none());
        Ok(())
    }
}
