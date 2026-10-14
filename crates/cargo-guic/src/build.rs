use crate::{plan::BuildPlan, process, report, staging};
use anyhow::{Context, Result, ensure};
use cargo_metadata::Message;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildRecord {
    pub schema_version: u32,
    pub id: String,
    pub package_id: String,
    pub bin: String,
    pub identifier: String,
    pub application_version: String,
    pub target: String,
    pub profile: String,
    pub features: Vec<String>,
    pub default_features: bool,
    pub locked: bool,
    pub executable_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub executable_hash: String,
    #[serde(default)]
    pub sidecars: Vec<crate::resources::Input>,
    pub build_affecting_config_hash: String,
    pub build_input_hashes: BTreeMap<PathBuf, Option<String>>,
    pub resource_layout: String,
    pub tool_versions: BTreeMap<String, String>,
    #[serde(default)]
    pub cli_executable_hash: Option<String>,
}

pub(crate) fn compile(plan: &BuildPlan) -> Result<BuildRecord> {
    let root = plan.root();
    let _lock = staging::lock(&root.join("application.lock"))?;
    process::hook(plan.config.hooks.before_build.as_ref())?;
    let refreshed = crate::plan::resolve(&plan.selectors, plan.development)?;
    ensure!(
        refreshed.static_config_hash == plan.static_config_hash,
        "before-build hook changed the build configuration; inspect and retry"
    );
    ensure!(
        refreshed.build_input_hashes.values().all(Option::is_some),
        "before-build hook did not generate all declared build.inputs"
    );
    let plan = &refreshed;
    let universal_directory = if plan.target == "universal-apple-darwin" {
        Some(
            tempfile::Builder::new()
                .prefix("universal-")
                .tempdir_in(&root)?,
        )
    } else {
        None
    };
    let executable = if let Some(directory) = &universal_directory {
        let arm = cargo_executable(plan, "aarch64-apple-darwin")?;
        let intel = cargo_executable(plan, "x86_64-apple-darwin")?;
        let output = directory.path().join(&plan.project.bin);
        merge_universal(&arm, &intel, &output)?;
        output
    } else {
        cargo_executable(plan, &plan.target)?
    };
    let snapshots = root.join("snapshots");
    std::fs::create_dir_all(&snapshots)?;
    let directory = tempfile::Builder::new()
        .prefix("build-")
        .tempdir_in(&snapshots)?;
    let filename = executable
        .file_name()
        .context("executable has no filename")?;
    let snapshot_path = directory.path().join(filename);
    std::fs::copy(&executable, &snapshot_path)?;
    let executable_hash = report::hash_file(&snapshot_path)?;
    ensure!(
        report::hash_file(&executable)? == executable_hash,
        "Cargo executable changed during snapshot creation; retry build"
    );
    let id = directory
        .path()
        .file_name()
        .context("snapshot directory has no name")?
        .to_string_lossy()
        .into_owned();
    let sidecar_directory = tempfile::Builder::new()
        .prefix("sidecars-")
        .tempdir_in(directory.path())?;
    let record = BuildRecord {
        schema_version: 1,
        id: id.clone(),
        package_id: plan.project.package_id.clone(),
        bin: plan.project.bin.clone(),
        identifier: plan.identifier.clone(),
        application_version: plan.project.version.clone(),
        target: plan.target.clone(),
        profile: plan.profile.clone(),
        features: plan.features.clone(),
        default_features: plan.default_features,
        locked: plan.locked,
        executable_path: executable,
        snapshot_path,
        executable_hash,
        sidecars: crate::sidecars::stage(
            &crate::sidecars::collect(plan)?,
            sidecar_directory.path(),
        )?,
        build_affecting_config_hash: plan.build_affecting_config_hash.clone(),
        build_input_hashes: plan.build_input_hashes.clone(),
        resource_layout: plan.resource_layout.clone(),
        tool_versions: plan.tool_versions.clone(),
        cli_executable_hash: Some(report::hash_file(&std::env::current_exe()?)?),
    };
    // Allocate a separate namespace so input copies cannot replace the executable.
    let input_directory = tempfile::Builder::new()
        .prefix("inputs-")
        .tempdir_in(directory.path())?;
    for (index, (path, hash)) in plan.build_input_hashes.iter().enumerate() {
        ensure!(
            hash.as_ref() == Some(&report::hash_file(path)?),
            "build input changed during compilation"
        );
        let input = input_directory.path().join(index.to_string());
        std::fs::copy(path, &input)?;
        ensure!(
            hash.as_ref() == Some(&report::hash_file(&input)?),
            "build input changed during snapshot creation"
        );
    }
    // Windows FlushFileBuffers requires a handle opened for writing.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&record.snapshot_path)?
        .sync_all()?;
    process::check_cancelled()?;
    report::atomic_new(&root.join("records").join(format!("{id}.json")), &record)?;
    let _ = input_directory.keep();
    let _ = sidecar_directory.keep();
    let _ = directory.keep();
    if let Some(directory) = universal_directory {
        let _ = directory.keep();
    }
    Ok(record)
}

fn cargo_executable(plan: &BuildPlan, target: &str) -> Result<PathBuf> {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(&plan.invocation_directory)
        .args(["build", "--message-format=json-render-diagnostics"])
        .arg("--manifest-path")
        .arg(&plan.project.manifest)
        .args([
            "--package",
            &plan.project.package,
            "--bin",
            &plan.project.bin,
            "--target",
            target,
            "--profile",
            &plan.profile,
        ]);
    if plan.locked {
        command.arg("--locked");
    }
    if !plan.features.is_empty() {
        command.arg("--features").arg(plan.features.join(","));
    }
    if !plan.default_features {
        command.arg("--no-default-features");
    }
    if let Some(target) = &plan.deployment_target {
        command.env("MACOSX_DEPLOYMENT_TARGET", target);
    }
    eprintln!(
        "Building {}:{} ({}, {})",
        plan.project.package, plan.project.bin, target, plan.profile
    );
    let handshake = if plan.os == "windows" {
        plan.config
            .build
            .windows_resources
            .as_ref()
            .map(|windows| -> Result<String> {
                let request = guic_build::Request {
                    schema_version: 1,
                    package: plan.project.package.clone(),
                    bin: plan.project.bin.clone(),
                    product_name: plan.product_name.clone(),
                    version: plan.project.version.clone(),
                    windows: windows.clone(),
                };
                // Include content hashes so resource changes invalidate Cargo's build script cache.
                let hash = report::hash_json(&(&request, &plan.build_input_hashes))?;
                command
                    .env("GUIC_BUILD_REQUEST", serde_json::to_string(&request)?)
                    .env("GUIC_BUILD_REQUEST_HASH", &hash);
                Ok(hash)
            })
            .transpose()?
    } else {
        None
    };
    if handshake.is_none() {
        command
            .env_remove("GUIC_BUILD_REQUEST")
            .env_remove("GUIC_BUILD_REQUEST_HASH");
    }
    command
        .env("GUIC_APP_PACKAGE", &plan.project.package)
        .env("GUIC_APP_IDENTIFIER", &plan.identifier)
        .env("GUIC_APP_NAME", &plan.product_name);
    let output = process::capture(&mut command)?;
    if let Some(hash) = handshake {
        verify_handshake(&output, &plan.project.package_id, &hash)?;
    }
    let executable = artifact(&output, &plan.project.package_id, &plan.project.bin)?;
    ensure!(
        executable.is_file(),
        "Cargo executable is missing: {}",
        executable.display()
    );
    Ok(executable)
}

fn merge_universal(arm: &Path, intel: &Path, output: &Path) -> Result<()> {
    let lipo =
        crate::platform::find("lipo").context("Universal Binary requires Apple's lipo tool")?;
    process::status(
        Command::new(&lipo)
            .arg("-create")
            .arg(arm)
            .arg(intel)
            .arg("-output")
            .arg(output),
    )?;
    process::status(
        Command::new(lipo)
            .arg(output)
            .args(["-verify_arch", "arm64", "x86_64"]),
    )
    .context("Universal Binary must contain both arm64 and x86_64")
}

pub(crate) fn artifact(output: &[u8], package_id: &str, bin: &str) -> Result<PathBuf> {
    let mut paths = std::collections::BTreeSet::new();
    let mut success = false;
    for message in Message::parse_stream(Cursor::new(output)) {
        match message.context("invalid Cargo JSON message")? {
            Message::CompilerArtifact(artifact)
                if artifact.package_id.repr == package_id
                    && artifact.target.name == bin
                    && artifact.target.is_bin()
                    && !artifact.profile.test =>
            {
                if let Some(path) = artifact.executable {
                    paths.insert(path.into_std_path_buf());
                }
            }
            Message::CompilerMessage(message) => {
                if let Some(rendered) = message.message.rendered {
                    eprint!("{rendered}");
                }
            }
            Message::BuildFinished(finished) => success = finished.success,
            Message::TextLine(line) => eprintln!("{line}"),
            _ => {}
        }
    }
    ensure!(success, "Cargo did not report a successful build");
    ensure!(
        paths.len() == 1,
        "Cargo must report exactly one executable for {package_id}:{bin}"
    );
    paths.into_iter().next().context("missing Cargo executable")
}

pub(crate) fn compatible(plan: &BuildPlan, record: &BuildRecord) -> bool {
    record.schema_version == 1
        && record.package_id == plan.project.package_id
        && record.bin == plan.project.bin
        && record.identifier == plan.identifier
        && record.application_version == plan.project.version
        && record.target == plan.target
        && record.profile == plan.profile
        && record.features == plan.features
        && record.default_features == plan.default_features
        && record.resource_layout == plan.resource_layout
        && record.build_affecting_config_hash == plan.build_affecting_config_hash
        && record.build_input_hashes == plan.build_input_hashes
}

pub(crate) fn verify(plan: &BuildPlan, record: &BuildRecord) -> Result<()> {
    ensure!(
        compatible(plan, record),
        "build record is incompatible with selected application, identity, version, target, profile, features or build configuration; rebuild or use matching selectors"
    );
    for (path, expected) in &plan.build_input_hashes {
        crate::config::reject_symlinks(path)?;
        ensure!(
            expected.as_ref() == Some(&report::hash_file(path)?),
            "build input changed after planning; rebuild before bundling"
        );
    }
    let expected_sidecars = crate::sidecars::collect(plan)?;
    ensure!(
        record.sidecars.len() == expected_sidecars.len(),
        "sidecar list changed; rebuild"
    );
    for (saved, expected) in record.sidecars.iter().zip(&expected_sidecars) {
        ensure!(
            saved.destination == expected.destination && saved.sha256 == expected.sha256,
            "sidecar changed; rebuild"
        );
        crate::config::reject_symlinks(&saved.source)?;
        ensure!(
            saved.source.canonicalize()?.starts_with(
                record
                    .snapshot_path
                    .parent()
                    .context("snapshot parent")?
                    .canonicalize()?
            ) && report::hash_file(&saved.source)? == saved.sha256,
            "invalid sidecar snapshot; rebuild"
        );
    }
    crate::config::reject_symlinks(&record.snapshot_path)?;
    let snapshots = plan
        .root()
        .join("snapshots")
        .canonicalize()
        .context("snapshot area is missing; rebuild")?;
    let path = record
        .snapshot_path
        .canonicalize()
        .context("build snapshot is missing; rebuild")?;
    ensure!(
        path.starts_with(snapshots) && path.is_file(),
        "snapshot is outside the application artifact area"
    );
    ensure!(
        report::hash_file(&path)? == record.executable_hash,
        "build snapshot has been modified; rebuild before bundling"
    );
    Ok(())
}

pub(crate) fn select_record(plan: &BuildPlan, explicit: Option<&Path>) -> Result<BuildRecord> {
    if let Some(path) = explicit {
        let record = report::read(path)?;
        verify(plan, &record)?;
        return Ok(record);
    }
    let directory = plan.root().join("records");
    ensure!(
        directory.is_dir(),
        "no build records; run cargo guic build --no-bundle first"
    );
    let mut records = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let record: BuildRecord = report::read(&path)
                .with_context(|| format!("invalid record {}", path.display()))?;
            if compatible(plan, &record) {
                records.push(record);
            }
        }
    }
    ensure!(
        records.len() == 1,
        "found {} compatible records; pass --build-record explicitly or rebuild",
        records.len()
    );
    let record = records.remove(0);
    verify(plan, &record)?;
    Ok(record)
}

#[cfg(test)]
pub(crate) fn fixture() -> Result<(tempfile::TempDir, BuildPlan, BuildRecord)> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let config: crate::config::Config =
        toml::from_str("schema-version=1\n[app]\nidentifier='org.example.fixture'\n")?;
    let plan = BuildPlan {
        schema_version: 1,
        project: crate::project::Project {
            package_id: "fixture-id".into(),
            package: "fixture".into(),
            bin: "fixture".into(),
            version: "1.0.0".into(),
            description: None,
            authors: vec![],
            license: None,
            homepage: None,
            directory: root.clone(),
            manifest: root.join("Cargo.toml"),
            workspace_root: root.clone(),
            target_directory: root.clone(),
        },
        config_path: root.join("Guic.toml"),
        invocation_directory: root.clone(),
        config,
        identifier: "org.example.fixture".into(),
        product_name: "Fixture".into(),
        target: "x86_64-unknown-linux-gnu".into(),
        host: "x86_64-unknown-linux-gnu".into(),
        os: "linux".into(),
        profile: "release".into(),
        features: vec![],
        default_features: true,
        locked: true,
        formats: vec!["deb".into(), "appimage".into()],
        deployment_target: None,
        resource_layout: "embedded-only-v1".into(),
        build_affecting_config_hash: "config-hash".into(),
        build_input_hashes: BTreeMap::new(),
        static_config_hash: "config-hash".into(),
        selectors: Default::default(),
        development: false,
        tool_versions: BTreeMap::new(),
        provenance: BTreeMap::new(),
    };
    let snapshot = plan.root().join("snapshots/build-fixture/fixture");
    std::fs::create_dir_all(snapshot.parent().context("snapshot parent")?)?;
    std::fs::write(&snapshot, b"executable fixture")?;
    let record = BuildRecord {
        schema_version: 1,
        id: "build-fixture".into(),
        package_id: plan.project.package_id.clone(),
        bin: plan.project.bin.clone(),
        identifier: plan.identifier.clone(),
        application_version: plan.project.version.clone(),
        target: plan.target.clone(),
        profile: plan.profile.clone(),
        features: vec![],
        default_features: true,
        locked: true,
        executable_path: root.join("original"),
        executable_hash: report::hash_file(&snapshot)?,
        sidecars: vec![],
        snapshot_path: snapshot,
        build_affecting_config_hash: plan.build_affecting_config_hash.clone(),
        build_input_hashes: BTreeMap::new(),
        resource_layout: "embedded-only-v1".into(),
        tool_versions: BTreeMap::new(),
        cli_executable_hash: None,
    };
    Ok((temporary, plan, record))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(package: &str, bin: &str, path: &str) -> serde_json::Value {
        serde_json::json!({"reason":"compiler-artifact", "package_id":package, "manifest_path":"/Cargo.toml",
            "target":{"name":bin,"kind":["bin"],"crate_types":["bin"],"src_path":"/main.rs","edition":"2021","doc":true,"doctest":false,"test":true},
            "profile":{"opt_level":"3","debuginfo":null,"debug_assertions":false,"overflow_checks":false,"test":false},
            "features":[],"filenames":[path],"executable":path,"fresh":false})
    }
    #[test]
    fn selects_only_matching_cargo_executable_and_requires_success() -> Result<()> {
        let output = format!(
            "{}\n{}\n{}\n{{\"reason\":\"build-finished\",\"success\":true}}\n",
            message("dependency", "app", "/wrong"),
            message("selected", "helper", "/also-wrong"),
            message("selected", "app", "/custom/path")
        );
        assert_eq!(
            artifact(output.as_bytes(), "selected", "app")?,
            PathBuf::from("/custom/path")
        );
        assert!(
            artifact(
                output
                    .replace("\"success\":true", "\"success\":false")
                    .as_bytes(),
                "selected",
                "app"
            )
            .is_err()
        );
        assert!(artifact(output.as_bytes(), "missing", "app").is_err());
        let ambiguous = format!("{output}\n{}\n", message("selected", "app", "/another"));
        assert!(artifact(ambiguous.as_bytes(), "selected", "app").is_err());
        Ok(())
    }
}

fn verify_handshake(output: &[u8], package: &str, expected: &str) -> Result<()> {
    for message in Message::parse_stream(Cursor::new(output)) {
        if let Message::BuildScriptExecuted(script) = message?
            && script.package_id.repr == package
            && script
                .env
                .iter()
                .any(|(key, value)| key == "GUIC_BUILD_REQUEST_HASH" && value == expected)
        {
            return Ok(());
        }
    }
    anyhow::bail!(
        "Windows resource integration was not acknowledged; add guic-build as a build dependency and call guic_build::build() in build.rs"
    )
}

#[cfg(test)]
mod handshake_tests {
    use super::*;
    #[test]
    fn acknowledgement_must_match_package_and_request() -> Result<()> {
        let message = serde_json::json!({"reason":"build-script-executed", "package_id":"app 1.0.0", "linked_libs":[], "linked_paths":[], "cfgs":[], "env":[["GUIC_BUILD_REQUEST_HASH","expected"]], "out_dir":"/tmp/out"});
        let bytes = serde_json::to_vec(&message)?;
        verify_handshake(&bytes, "app 1.0.0", "expected")?;
        assert!(verify_handshake(&bytes, "dependency 1.0.0", "expected").is_err());
        assert!(verify_handshake(&bytes, "app 1.0.0", "stale").is_err());
        assert!(verify_handshake(b"", "app 1.0.0", "expected").is_err());
        Ok(())
    }
}
