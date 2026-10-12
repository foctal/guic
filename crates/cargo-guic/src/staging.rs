use crate::{
    backend::{self, Request, Response},
    build::{self, BuildRecord},
    config,
    plan::BuildPlan,
    process,
    report::{self, Artifact, ArtifactReport},
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub(crate) fn lock(path: &Path) -> Result<File> {
    std::fs::create_dir_all(path.parent().context("lock has no parent")?)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    loop {
        process::check_cancelled()?;
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(file),
            Err(e) if e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub(crate) fn bundle(plan: &BuildPlan, record: &BuildRecord) -> Result<ArtifactReport> {
    backend::validate(plan)?;
    bundle_with(plan, record, false, false, backend::execute)
}

pub(crate) fn trial(
    plan: &BuildPlan,
    record: &BuildRecord,
    execute_signing: bool,
) -> Result<ArtifactReport> {
    backend::validate_trial(plan, execute_signing)?;
    bundle_with(plan, record, true, execute_signing, backend::execute)
}

fn bundle_with(
    plan: &BuildPlan,
    record: &BuildRecord,
    trial: bool,
    execute_signing: bool,
    mut execute: impl FnMut(&Request, &Path) -> Result<Response>,
) -> Result<ArtifactReport> {
    let root = plan.root();
    let _application_lock = lock(&root.join("application.lock"))?;
    // All invocations sharing a backend cache serialize, even for different applications.
    let cache = dirs::cache_dir().context("cannot resolve backend cache directory")?;
    let _backend_lock = lock(&cache.join("cargo-guic").join("cargo-packager.lock"))?;
    build::verify(plan, record)?;
    crate::runtime::validate_binary_policy(plan, &record.snapshot_path)?;
    process::hook(plan.config.hooks.before_bundle.as_ref())?;
    build::verify(plan, record)?;
    let attempts = root.join("artifacts");
    std::fs::create_dir_all(&attempts)?;
    // Preserve incomplete attempts and successful outputs for diagnosis.
    let attempt = tempfile::Builder::new()
        .prefix("attempt-")
        .tempdir_in(attempts)?
        .keep();
    let mut report = ArtifactReport {
        schema_version: 1,
        identifier: plan.identifier.clone(),
        version: plan.project.version.clone(),
        build_record_id: record.id.clone(),
        target: record.target.clone(),
        profile: record.profile.clone(),
        features: record.features.clone(),
        default_features: record.default_features,
        requested_formats: plan.formats.clone(),
        resource_hashes: BTreeMap::new(),
        tool_versions: plan.tool_versions.clone(),
        outputs: Vec::new(),
        code_signing: "disabled".into(),
        notarization: "disabled".into(),
        stapling: "disabled".into(),
        complete: false,
        failed_format: None,
        trial,
        acceptance: "not-verified".into(),
        cli_executable_hash: Some(report::hash_file(&std::env::current_exe()?)?),
    };
    let mut staged_icons = Vec::new();
    let mut destinations = BTreeSet::new();
    for icon in &plan.config.bundle.icons {
        let destination =
            PathBuf::from("icons").join(icon.file_name().context("icon has no filename")?);
        reserve(&mut destinations, &destination)?;
        let path = copy_input(icon, &attempt.join("resource-snapshot").join(&destination))?;
        report
            .resource_hashes
            .insert(destination.clone(), report::hash_file(&path)?);
        staged_icons.push((destination, path));
    }
    let external = crate::resources::collect(&plan.config.bundle.resources)?;
    crate::resources::validate_installed_destinations(plan, &external)?;
    let external = crate::resources::stage(&external, &attempt.join("external-resource-snapshot"))?;
    for resource in &external {
        report.resource_hashes.insert(
            PathBuf::from("resources").join(&resource.destination),
            resource.sha256.clone(),
        );
    }
    let (customization, hashes) = crate::resources::stage_backend(
        &plan.config.backend.cargo_packager,
        &attempt.join("backend-snapshot"),
    )?;
    report.resource_hashes.extend(hashes);
    if plan.config.signing.code_signing == config::Policy::Required {
        report.code_signing = "not-verified".into();
    }
    if plan.config.signing.notarization == config::Policy::Required {
        report.notarization = "not-verified".into();
        report.stapling = "not-verified".into();
    }
    for format in &plan.formats {
        report.failed_format = Some(format.clone());
        let result = (|| -> Result<Vec<Artifact>> {
            process::check_cancelled()?;
            let directory = attempt.join(format);
            let input = directory.join("input");
            let output = directory.join("output");
            std::fs::create_dir_all(&input)?;
            std::fs::create_dir_all(&output)?;
            let executable = input.join(
                record
                    .snapshot_path
                    .file_name()
                    .context("snapshot has no filename")?,
            );
            std::fs::copy(&record.snapshot_path, &executable)?;
            ensure!(
                report::hash_file(&executable)? == record.executable_hash,
                "snapshot changed while staging"
            );
            let mut icons = Vec::new();
            for (destination, source) in &staged_icons {
                icons.push(copy_input(source, &input.join(destination))?);
            }
            let request = Request {
                schema_version: 1,
                target: record.target.clone(),
                format: format.clone(),
                identifier: plan.identifier.clone(),
                product_name: plan.product_name.clone(),
                version: plan.project.version.clone(),
                bin: record.bin.clone(),
                executable,
                output_directory: output.clone(),
                icons,
                deployment_target: plan.deployment_target.clone(),
                description: plan.project.description.clone(),
                authors: plan.project.authors.clone(),
                homepage: plan.project.homepage.clone(),
                customization: crate::resources::stage_backend(
                    &customization,
                    &input.join("backend"),
                )?
                .0,
                resources: crate::resources::stage(&external, &input.join("resources"))?,
                trial,
                execute_signing,
                signing: plan.config.signing.clone(),
                file_associations: plan.config.bundle.file_associations.clone(),
                protocols: plan.config.bundle.protocols.clone(),
            };
            let result = execute(&request, &directory)?;
            let artifacts = verify_outputs(format, &output, &result)?;
            for artifact in &artifacts {
                if plan.config.signing.code_signing == config::Policy::Required {
                    ensure!(
                        artifact.verification.code_signing == crate::signing::Outcome::Verified,
                        "required code signing was not verified"
                    );
                }
                if plan.config.signing.notarization == config::Policy::Required {
                    ensure!(
                        artifact.verification.notarization == crate::signing::Outcome::Verified
                            && artifact.verification.stapling == crate::signing::Outcome::Verified,
                        "required notarization and stapling were not verified"
                    );
                }
            }
            Ok(artifacts)
        })();
        match result {
            Ok(outputs) => report.outputs.extend(outputs),
            Err(error) => {
                // Cancellation preserves the attempt directory and must never emit success.
                if process::check_cancelled().is_ok() {
                    report::atomic_new(&attempt.join("incomplete.json"), &report)?;
                }
                return Err(error).with_context(|| format!("packaging stage: {} target {} format {}; incomplete artifacts preserved in {}",
                    plan.identifier, record.target, format, attempt.display()));
            }
        }
    }
    report.failed_format = None;
    report.complete = true;
    if plan.config.signing.code_signing == config::Policy::Required {
        report.code_signing = "verified".into();
    }
    if plan.config.signing.notarization == config::Policy::Required {
        report.notarization = "verified".into();
        report.stapling = "verified".into();
    }
    report::atomic_new(&attempt.join("report.json"), &report)?;
    Ok(report)
}

pub(crate) fn reserve(destinations: &mut BTreeSet<String>, destination: &Path) -> Result<()> {
    ensure!(
        !destination.as_os_str().is_empty()
            && destination
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "resource destination must be a nonempty relative path without traversal"
    );
    let normalized = destination
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    ensure!(
        !normalized.contains(':')
            && !normalized.split('/').any(|p| {
                let stem = p.split('.').next().unwrap_or("");
                p == "."
                    || p == ".."
                    || p.is_empty()
                    || p.ends_with(['.', ' '])
                    || ["con", "prn", "aux", "nul"].contains(&stem)
                    || (stem.len() == 4
                        && (stem.starts_with("com") || stem.starts_with("lpt"))
                        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
            }),
        "unsafe resource destination"
    );
    ensure!(
        !destinations.iter().any(|existing| existing == &normalized
            || existing.starts_with(&format!("{normalized}/"))
            || normalized.starts_with(&format!("{existing}/"))),
        "resource destination collision: {}",
        destination.display()
    );
    destinations.insert(normalized);
    Ok(())
}

fn copy_input(source: &Path, destination: &Path) -> Result<PathBuf> {
    config::reject_symlinks(source)?;
    ensure!(source.is_file(), "staged input must be a regular file");
    std::fs::create_dir_all(destination.parent().context("input has no parent")?)?;
    std::fs::copy(source, destination)?;
    ensure!(
        report::hash_file(source)? == report::hash_file(destination)?,
        "input changed during staging"
    );
    Ok(destination.to_owned())
}

fn verify_outputs(format: &str, directory: &Path, response: &Response) -> Result<Vec<Artifact>> {
    ensure!(
        response.schema_version == 1 && response.error.is_none(),
        "unsupported backend response version"
    );
    let directory = directory.canonicalize()?;
    let matching: Vec<_> = response
        .outputs
        .iter()
        .filter(|output| output.format == format)
        .collect();
    ensure!(
        matching.len() == 1 && !matching[0].paths.is_empty(),
        "backend did not produce requested format {format}"
    );
    let mut artifacts = Vec::new();
    let mut seen = BTreeSet::new();
    for path in &matching[0].paths {
        config::reject_symlinks(path)?;
        let path = path
            .canonicalize()
            .context("backend reported a missing output")?;
        ensure!(
            path.starts_with(&directory) && path != directory && seen.insert(path.clone()),
            "backend output escapes operation directory or is duplicated"
        );
        let extension = match format {
            "appimage" => "AppImage",
            "nsis" => "exe",
            other => other,
        };
        ensure!(
            path.extension().is_some_and(|ext| ext == extension),
            "backend output has unexpected extension"
        );
        ensure!(
            if format == "app" {
                path.is_dir()
            } else {
                path.is_file()
            },
            "backend output has unexpected type"
        );
        let (size, sha256) = if path.is_file() {
            (path.metadata()?.len(), report::hash_file(&path)?)
        } else {
            hash_directory(&path)?
        };
        ensure!(size > 0, "backend output is empty");
        artifacts.push(Artifact {
            format: format.into(),
            path,
            size,
            sha256,
            verification: matching[0].verification.clone(),
            kind: if format == "app" { "directory" } else { "file" }.into(),
        });
    }
    Ok(artifacts)
}

pub(crate) fn hash_directory(path: &Path) -> Result<(u64, String)> {
    let mut files = BTreeMap::new();
    let mut size = 0;
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            let link = std::fs::read_link(entry.path())?;
            crate::resources::validate_link(path, entry.path(), &link)?;
            files.insert(
                entry.path().strip_prefix(path)?.to_owned(),
                format!("link:{}", link.to_string_lossy()),
            );
            continue;
        }
        if entry.file_type().is_file() {
            size += entry.metadata()?.len();
            files.insert(
                entry.path().strip_prefix(path)?.to_owned(),
                report::hash_file(entry.path())?,
            );
        } else {
            ensure!(entry.file_type().is_dir(), "non-regular bundle output");
        }
    }
    Ok((size, report::hash_json(&files)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contended_lock_waits_until_the_owner_releases_it() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("application.lock");
        let owner = lock(&path)?;
        let (sender, receiver) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let result = lock(&path);
            sender.send(result).expect("lock result receiver");
        });
        let pending = receiver.recv_timeout(Duration::from_millis(100));
        drop(owner);
        waiter.join().expect("lock waiter");
        assert!(matches!(
            pending,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        let _acquired = receiver.recv()??;
        Ok(())
    }

    #[test]
    fn partial_failure_keeps_earlier_outputs_and_never_modifies_snapshot() -> Result<()> {
        let (_temporary, plan, record) = build::fixture()?;
        let original_hash = report::hash_file(&record.snapshot_path)?;
        let result = bundle_with(&plan, &record, true, false, |request, _| {
            // Simulate a backend that signs or mutates its input executable.
            std::fs::write(&request.executable, b"backend mutation")?;
            if request.format == "appimage" {
                anyhow::bail!("simulated tool failure");
            }
            let artifact = request.output_directory.join("fixture.deb");
            std::fs::write(&artifact, b"completed first format")?;
            Ok(Response {
                schema_version: 1,
                error: None,
                outputs: vec![backend::Output {
                    format: "deb".into(),
                    paths: vec![artifact],
                    verification: Default::default(),
                }],
            })
        });
        let error = result.expect_err("second format must fail");
        assert!(format!("{error:#}").contains("simulated tool failure"));
        assert_eq!(report::hash_file(&record.snapshot_path)?, original_hash);
        let attempt = std::fs::read_dir(plan.root().join("artifacts"))?
            .next()
            .context("attempt")??
            .path();
        assert!(!attempt.join("report.json").exists());
        let partial: ArtifactReport = report::read(&attempt.join("incomplete.json"))?;
        assert!(!partial.complete);
        assert_eq!(partial.failed_format.as_deref(), Some("appimage"));
        assert_eq!(partial.outputs.len(), 1);
        assert!(partial.outputs[0].path.is_file());
        Ok(())
    }

    #[test]
    fn collisions_and_escaping_paths_are_rejected() -> Result<()> {
        let mut paths = BTreeSet::new();
        reserve(&mut paths, Path::new("assets/icon.png"))?;
        for invalid in [
            "assets/Icon.PNG",
            "assets",
            "assets/icon.png/child",
            "../escape",
            "/absolute",
            "C:\\escape",
            "..\\escape",
            "assets/./icon.png",
            "NUL",
            "a/COM1.txt",
            "a/trailing.",
        ] {
            assert!(
                reserve(&mut paths, Path::new(invalid)).is_err(),
                "{invalid}"
            );
        }
        Ok(())
    }
    #[test]
    fn missing_wrong_and_empty_backend_outputs_fail() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let missing = Response {
            schema_version: 1,
            error: None,
            outputs: Vec::new(),
        };
        assert!(verify_outputs("deb", dir.path(), &missing).is_err());
        let path = dir.path().canonicalize()?.join("app.deb");
        let response = Response {
            schema_version: 1,
            error: None,
            outputs: vec![backend::Output {
                format: "deb".into(),
                paths: vec![path.clone()],
                verification: Default::default(),
            }],
        };
        assert!(verify_outputs("deb", dir.path(), &response).is_err());
        std::fs::write(&path, [])?;
        assert!(verify_outputs("deb", dir.path(), &response).is_err());
        std::fs::write(&path, b"fixture")?;
        assert_eq!(verify_outputs("deb", dir.path(), &response)?.len(), 1);
        assert!(verify_outputs("nsis", dir.path(), &response).is_err());
        Ok(())
    }
    #[test]
    fn required_verification_cannot_be_claimed_by_a_successful_backend() -> Result<()> {
        let (_temporary, mut plan, record) = build::fixture()?;
        plan.formats = vec!["deb".into()];
        plan.config.signing.code_signing = config::Policy::Required;
        let result = bundle_with(&plan, &record, true, true, |request, _| {
            let artifact = request.output_directory.join("fixture.deb");
            std::fs::write(&artifact, b"unsigned output")?;
            Ok(Response {
                schema_version: 1,
                error: None,
                outputs: vec![backend::Output {
                    format: "deb".into(),
                    paths: vec![artifact],
                    verification: Default::default(),
                }],
            })
        });
        assert!(result.is_err());
        let attempt = std::fs::read_dir(plan.root().join("artifacts"))?
            .next()
            .context("attempt")??
            .path();
        let partial: ArtifactReport = report::read(&attempt.join("incomplete.json"))?;
        assert_eq!(partial.code_signing, "not-verified");
        assert!(!partial.complete);
        Ok(())
    }
}
