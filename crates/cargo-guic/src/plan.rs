use crate::{
    cli::Selectors,
    config::{self, Config},
    project::{self, Project},
    report,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BuildPlan {
    pub schema_version: u32,
    pub project: Project,
    pub config_path: PathBuf,
    pub invocation_directory: PathBuf,
    #[serde(skip)]
    pub config: Config,
    pub identifier: String,
    pub product_name: String,
    pub target: String,
    pub host: String,
    pub os: String,
    pub profile: String,
    pub features: Vec<String>,
    pub default_features: bool,
    pub locked: bool,
    pub formats: Vec<String>,
    pub deployment_target: Option<String>,
    pub resource_layout: String,
    pub build_affecting_config_hash: String,
    pub build_input_hashes: BTreeMap<PathBuf, Option<String>>,
    #[serde(skip)]
    pub static_config_hash: String,
    #[serde(skip)]
    pub selectors: Selectors,
    #[serde(skip)]
    pub development: bool,
    pub tool_versions: BTreeMap<String, String>,
    pub provenance: BTreeMap<String, String>,
}

impl BuildPlan {
    pub fn root(&self) -> PathBuf {
        self.project
            .target_directory
            .join("guic")
            .join(report::digest(self.project.package_id.as_bytes()))
    }

    pub fn inspection(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1, "plan": self,
            "resource_layout": self.resource_layout,
            "bundle": self.config.bundle,
            "signing": self.config.signing,
            "runtime": self.config.runtime,
            "windows_resources": self.config.build.windows_resources,
            "backend": self.config.backend,
            "hooks": {
                "before_build": self.config.hooks.before_build.is_some(),
                "before_bundle": self.config.hooks.before_bundle.is_some(),
                "arguments": "omitted from inspection"
            },
            "packaging_acceptance": "native packaging supported; installation acceptance remains application-specific",
            "build_record_scope": "Known build conditions, not proof of current source or reproducibility"
        })
    }
}

pub(crate) fn resolve(selectors: &Selectors, dev: bool) -> Result<BuildPlan> {
    let invocation_directory = std::env::current_dir()?.canonicalize()?;
    let cargo_config = cargo_config2::Config::load_with_cwd(&invocation_directory)
        .map_err(|_| anyhow::anyhow!("could not resolve Cargo configuration"))?;
    let host = cargo_config.host_triple()?.to_owned();
    let initial_target = cargo_config.build_target_for_cli(selectors.target.iter())?;
    ensure!(
        initial_target.len() <= 1,
        "multiple Cargo build targets are not supported; select --target"
    );
    let preliminary = initial_target.first().unwrap_or(&host);
    let os = target_os(preliminary)?;
    let metadata = project::metadata(selectors)?;
    // An explicit config can supply the package selector in a virtual workspace.
    let explicit = selectors
        .config
        .as_ref()
        .map(|p| p.canonicalize())
        .transpose()?;
    let preloaded = explicit.as_ref().map(|p| config::load(p, os)).transpose()?;
    let requested_package = selectors.package.as_deref().or_else(|| {
        preloaded
            .as_ref()
            .and_then(|l| l.config.build.package.as_deref())
    });
    let package = project::select(&metadata, requested_package)?;
    let config_path = explicit.unwrap_or_else(|| {
        package
            .manifest_path
            .parent()
            .expect("Cargo manifest has a parent")
            .join("Guic.toml")
            .into_std_path_buf()
    });
    let mut loaded = match preloaded {
        Some(loaded) => loaded,
        None => config::load(&config_path, os)?,
    };
    let target = selectors
        .target
        .clone()
        .or_else(|| loaded.config.build.target.clone())
        .or_else(|| initial_target.first().cloned())
        .unwrap_or_else(|| host.clone());
    let final_os = target_os(&target)?;
    if final_os != os {
        loaded = config::load(&config_path, final_os)?;
        ensure!(
            selectors.target.is_some() || loaded.config.build.target.as_deref() == Some(&target),
            "platform overrides must not change target operating system recursively; use --target"
        );
    }
    let config = loaded.config;
    ensure!(
        target == host || (target == "universal-apple-darwin" && host.ends_with("-apple-darwin")),
        "target {target} is not the native host {host}; cross-compilation is not yet validated"
    );
    if final_os == "windows" {
        ensure!(
            target.ends_with("-msvc"),
            "only Windows MSVC targets are supported"
        );
    }
    if let Some(name) = &config.build.package {
        ensure!(
            name == package.name.as_str() || name == &package.id.repr,
            "configured build.package conflicts with selected package {}",
            package.name
        );
    }
    if let (Some(cli), Some(config)) = (&selectors.bin, &config.build.bin) {
        ensure!(cli == config, "--bin conflicts with build.bin");
    }
    let project = project::resolve(
        &metadata,
        package,
        selectors.bin.as_deref().or(config.build.bin.as_deref()),
    )?;
    let profile = selectors
        .profile
        .clone()
        .or_else(|| config.build.profile.clone())
        .unwrap_or_else(|| if dev { "dev" } else { "release" }.into());
    ensure!(
        !profile.is_empty()
            && profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid profile name"
    );
    let mut features: Vec<String> = selectors
        .features
        .clone()
        .or_else(|| config.build.features.clone())
        .unwrap_or_default()
        .iter()
        .flat_map(|value| value.split([',', ' ']))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    features.sort();
    features.dedup();
    let default_features =
        !selectors.no_default_features && config.build.default_features.unwrap_or(true);
    let formats = selectors
        .formats
        .clone()
        .unwrap_or_else(|| config.bundle.formats.for_os(final_os));
    let mut seen = std::collections::BTreeSet::new();
    for format in &formats {
        ensure!(seen.insert(format), "duplicate package format: {format}");
        ensure!(
            match final_os {
                "macos" => ["app", "dmg"].contains(&format.as_str()),
                "windows" => format == "nsis",
                _ => ["deb", "appimage"].contains(&format.as_str()),
            },
            "format {format} is unsupported on {final_os}"
        );
    }
    let deployment_target = config.build.deployment_target.clone().or_else(|| {
        cargo_config
            .env
            .get("MACOSX_DEPLOYMENT_TARGET")
            .filter(|v| v.force || std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none())
            .map(|v| v.value.to_string_lossy().into_owned())
            .or_else(|| std::env::var("MACOSX_DEPLOYMENT_TARGET").ok())
    });
    if let Some(version) = &deployment_target {
        ensure!(
            final_os == "macos"
                && version.split('.').count() >= 2
                && version
                    .split('.')
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "deployment-target requires a macOS numeric version"
        );
        if let Some(forced) = cargo_config
            .env
            .get("MACOSX_DEPLOYMENT_TARGET")
            .filter(|v| v.force)
        {
            ensure!(
                forced.value == std::ffi::OsStr::new(version),
                "forced Cargo deployment target conflicts with Guic.toml"
            );
        }
    }
    let mut tool_versions = BTreeMap::new();
    tool_versions.insert("cargo-guic".into(), env!("CARGO_PKG_VERSION").into());
    tool_versions.insert("cargo-packager".into(), crate::backend::VERSION.into());
    let mut rustc = std::process::Command::from(cargo_config.rustc());
    rustc.arg("-vV");
    let rustc_version = String::from_utf8(crate::process::capture(&mut rustc)?)?;
    tool_versions.insert("rustc".into(), rustc_version.trim().into());
    let mut cargo = std::process::Command::new(cargo_config.cargo());
    cargo.arg("-V");
    tool_versions.insert(
        "cargo".into(),
        String::from_utf8(crate::process::capture(&mut cargo)?)?
            .trim()
            .into(),
    );
    let environment: BTreeMap<_, _> = std::env::vars()
        .filter(|(k, _)| {
            k.starts_with("CARGO_")
                || k.starts_with("RUST")
                || k.starts_with("CFLAGS")
                || k.starts_with("CXXFLAGS")
                || ["CC", "CXX", "AR", "SDKROOT", "MACOSX_DEPLOYMENT_TARGET"].contains(&k.as_str())
        })
        .collect();
    // Persist only a digest of compiler configuration and environment, never credentials or flags.
    let resource_layout = match config.bundle.resource_contract {
        Some(config::ResourceContract::ApplicationManagedV1) => "application-managed-v1",
        Some(config::ResourceContract::GuicNativeV1) => "guic-native-v1",
        None => "embedded-only-v1",
    }
    .to_owned();
    // Validate source expansion and destinations without running project hooks.
    crate::resources::collect(&config.bundle.resources)?;
    let static_config_hash = report::hash_json(&serde_json::json!({
        "package": project.package_id, "bin": project.bin, "target": target, "profile": profile,
        "features": features, "default_features": default_features, "deployment_target": deployment_target,
        "windows_product_name": if final_os == "windows" && config.build.windows_resources.is_some() { config.app.product_name.as_ref() } else { None },
        "windows_resources": if final_os == "windows" { config.build.windows_resources.as_ref() } else { None },
        "before_build": config.hooks.before_build, "cargo_config": serde_json::to_value(&cargo_config)?,
        "environment": environment, "resource_layout": resource_layout, "tools": tool_versions,
        "cargo_configuration_files": cargo_configuration_hashes(&invocation_directory)?,
        "package_manifest": report::hash_file(&project.manifest)?,
        "workspace_manifest": report::hash_file(&project.workspace_root.join("Cargo.toml"))?,
        "hook_program": config.hooks.before_build.as_ref().filter(|h| std::path::Path::new(&h.program).is_file())
            .map(|h| report::hash_file(std::path::Path::new(&h.program))).transpose()?
    }))?;
    let mut build_input_hashes: BTreeMap<_, _> = config
        .build
        .inputs
        .iter()
        .map(|path| {
            Ok((
                path.clone(),
                if path.exists() {
                    Some(report::hash_file(path)?)
                } else {
                    None
                },
            ))
        })
        .collect::<Result<_>>()?;
    if final_os == "windows"
        && let Some(windows) = &config.build.windows_resources
    {
        for path in [&windows.icon, &windows.manifest].into_iter().flatten() {
            build_input_hashes.insert(path.clone(), Some(report::hash_file(path)?));
        }
    }
    let build_affecting_config_hash = report::hash_json(&serde_json::json!({
        "configuration": static_config_hash, "inputs": build_input_hashes
    }))?;
    let mut provenance = loaded.provenance;
    for (field, explicit, configured, fallback) in [
        (
            "build.target",
            selectors.target.is_some(),
            config.build.target.is_some(),
            "cargo configuration or host",
        ),
        (
            "build.profile",
            selectors.profile.is_some(),
            config.build.profile.is_some(),
            "command default",
        ),
        (
            "build.features",
            selectors.features.is_some(),
            config.build.features.is_some(),
            "default",
        ),
        (
            "build.default-features",
            selectors.no_default_features,
            config.build.default_features.is_some(),
            "default",
        ),
        (
            "bundle.formats",
            selectors.formats.is_some(),
            true,
            "config",
        ),
    ] {
        if explicit {
            provenance.insert(field.into(), "cli".into());
        } else if !configured {
            provenance.insert(field.into(), fallback.into());
        }
    }
    for (field, explicit, configured) in [
        (
            "build.package",
            selectors.package.is_some(),
            config.build.package.is_some(),
        ),
        (
            "build.bin",
            selectors.bin.is_some(),
            config.build.bin.is_some(),
        ),
    ] {
        if explicit {
            provenance.insert(field.into(), "cli".into());
        } else if !configured {
            provenance.insert(field.into(), "cargo metadata".into());
        }
    }
    if config.app.product_name.is_none() {
        provenance.insert("app.product-name".into(), "cargo package name".into());
    }
    if config.build.deployment_target.is_none() && deployment_target.is_some() {
        provenance.insert(
            "build.deployment-target".into(),
            "cargo configuration or environment".into(),
        );
    }
    let plan = BuildPlan {
        schema_version: 1,
        identifier: config
            .app
            .identifier
            .clone()
            .context("missing identifier")?,
        product_name: config
            .app
            .product_name
            .clone()
            .unwrap_or_else(|| project.package.clone()),
        project,
        config_path: config_path.canonicalize()?,
        invocation_directory,
        target,
        host,
        os: final_os.into(),
        profile,
        features,
        default_features,
        locked: !selectors.unlocked,
        formats,
        deployment_target,
        resource_layout,
        build_affecting_config_hash,
        build_input_hashes,
        static_config_hash,
        selectors: selectors.clone(),
        development: dev,
        tool_versions,
        provenance,
        config,
    };
    crate::resources::validate_installed_destinations(
        &plan,
        &crate::resources::collect(&plan.config.bundle.resources)?,
    )?;
    Ok(plan)
}

fn cargo_configuration_hashes(cwd: &std::path::Path) -> Result<BTreeMap<PathBuf, String>> {
    let mut paths: Vec<_> = cwd.ancestors().map(|p| p.join(".cargo")).collect();
    if let Some(home) = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")))
    {
        paths.push(home);
    }
    let mut hashes = BTreeMap::new();
    for directory in paths {
        for name in ["config", "config.toml"] {
            let path = directory.join(name);
            if path.is_file() {
                hashes.insert(path.clone(), report::hash_file(&path)?);
            }
        }
    }
    Ok(hashes)
}

pub(crate) fn target_os(target: &str) -> Result<&'static str> {
    if target.ends_with("-apple-darwin") {
        Ok("macos")
    } else if target.contains("-windows-") {
        Ok("windows")
    } else if target.contains("-linux-") {
        Ok("linux")
    } else {
        anyhow::bail!("unsupported desktop target: {target}")
    }
}
