use crate::{
    cli::{Init, Selectors},
    config, process,
};
use anyhow::{Context, Result, ensure};
use cargo_metadata::{Metadata, Package};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Command};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Project {
    pub package_id: String,
    pub package: String,
    pub bin: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub directory: PathBuf,
    pub manifest: PathBuf,
    pub workspace_root: PathBuf,
    pub target_directory: PathBuf,
}

pub(crate) fn metadata(selectors: &Selectors) -> Result<Metadata> {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.args(["metadata", "--format-version=1", "--no-deps"]);
    let manifest = selectors.manifest_path.clone().or_else(|| {
        selectors.config.as_ref().map(|p| {
            p.parent()
                .unwrap_or(std::path::Path::new("."))
                .join("Cargo.toml")
        })
    });
    if let Some(manifest) = manifest {
        command.arg("--manifest-path").arg(manifest);
    }
    if !selectors.unlocked {
        command.arg("--locked");
    }
    serde_json::from_slice(&process::capture(&mut command)?).context("invalid Cargo metadata")
}

pub(crate) fn select<'a>(metadata: &'a Metadata, package: Option<&str>) -> Result<&'a Package> {
    let candidates: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| {
            metadata.workspace_members.contains(&p.id)
                && p.targets.iter().any(|t| t.is_bin())
                && package.is_none_or(|name| p.name.as_str() == name || p.id.repr == name)
        })
        .collect();
    if package.is_none()
        && let Some(root) = metadata
            .root_package()
            .filter(|p| p.targets.iter().any(|t| t.is_bin()))
    {
        return Ok(root);
    }
    ensure!(
        candidates.len() == 1,
        "select one application with --package; eligible packages: {}",
        candidates
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(candidates[0])
}

pub(crate) fn resolve(
    metadata: &Metadata,
    package: &Package,
    bin: Option<&str>,
) -> Result<Project> {
    let chosen = bin.or(package.default_run.as_deref());
    let bins: Vec<_> = package
        .targets
        .iter()
        .filter(|t| t.is_bin() && chosen.is_none_or(|b| t.name == b))
        .collect();
    ensure!(
        bins.len() == 1,
        "package {} requires an unambiguous --bin; available binaries: {}",
        package.name,
        package
            .targets
            .iter()
            .filter(|t| t.is_bin())
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let manifest = package.manifest_path.clone().into_std_path_buf();
    Ok(Project {
        package_id: package.id.repr.clone(),
        package: package.name.to_string(),
        bin: bins[0].name.clone(),
        version: package.version.to_string(),
        description: package.description.clone(),
        authors: package.authors.clone(),
        license: package.license.clone(),
        homepage: package.homepage.clone(),
        directory: manifest
            .parent()
            .context("manifest has no parent")?
            .to_owned(),
        manifest,
        workspace_root: metadata.workspace_root.clone().into_std_path_buf(),
        target_directory: metadata.target_directory.clone().into_std_path_buf(),
    })
}

pub(crate) fn init(args: &Init) -> Result<serde_json::Value> {
    ensure!(
        args.selectors.target.is_none()
            && args.selectors.profile.is_none()
            && args.selectors.features.is_none()
            && !args.selectors.no_default_features
            && args.selectors.formats.is_none(),
        "init accepts project selectors only; set build options in the generated Guic.toml"
    );
    config::validate_identifier(&args.identifier)?;
    let metadata = metadata(&args.selectors)?;
    let package = select(&metadata, args.selectors.package.as_deref())?;
    let project = resolve(&metadata, package, args.selectors.bin.as_deref())?;
    let name = args.product_name.as_deref().unwrap_or(&project.package);
    config::validate_name(name)?;
    let path = args
        .selectors
        .config
        .clone()
        .unwrap_or_else(|| project.directory.join("Guic.toml"));
    let config = config::Config {
        schema_version: 1,
        app: config::App {
            identifier: Some(args.identifier.clone()),
            product_name: Some(name.into()),
        },
        build: config::Build {
            package: Some(project.package),
            bin: Some(project.bin),
            ..Default::default()
        },
        bundle: Default::default(),
        hooks: Default::default(),
        signing: Default::default(),
        runtime: Default::default(),
        backend: Default::default(),
        platform: Default::default(),
    };
    let text = toml::to_string_pretty(&config)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .context("cannot create Guic.toml; existing files are never overwritten")?;
    use std::io::Write;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(
        serde_json::json!({"schema_version": 1, "status": "complete", "config": path.canonicalize()?}),
    )
}
