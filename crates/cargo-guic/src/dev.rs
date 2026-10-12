use crate::{build, plan::BuildPlan, process, report, resources};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

pub(crate) fn run(
    plan: &BuildPlan,
    args: &[String],
    watch: bool,
    paths: &[PathBuf],
    ignores: &[PathBuf],
) -> Result<serde_json::Value> {
    if !watch {
        let (record, _) = launch(plan, args, || Ok(false))?;
        return Ok(serde_json::to_value(record)?);
    }
    let mut roots = watch_roots(plan, paths);
    let mut excluded = vec![plan.project.target_directory.clone()];
    excluded.extend(ignores.iter().map(|p| plan.invocation_directory.join(p)));
    let mut previous = fingerprint(&roots, &excluded)?;
    loop {
        process::check_cancelled()?;
        let current = crate::plan::resolve(&plan.selectors, true);
        let result = current.and_then(|plan| {
            roots = watch_roots(&plan, paths);
            if !excluded.contains(&plan.project.target_directory) {
                excluded.push(plan.project.target_directory.clone());
            }
            launch(&plan, args, || {
                let current = fingerprint(&roots, &excluded)?;
                if current != previous {
                    // Wait for a short quiet period before stopping and rebuilding.
                    std::thread::sleep(Duration::from_millis(150));
                    previous = fingerprint(&roots, &excluded)?;
                    return Ok(true);
                }
                Ok(false)
            })
        });
        process::check_cancelled()?;
        match result {
            Ok((_, true)) => continue,
            Ok((_, false)) => eprintln!("Application exited; watching for changes"),
            Err(error) => eprintln!("Development attempt failed: {error:#}; watching for changes"),
        }
        loop {
            process::check_cancelled()?;
            std::thread::sleep(Duration::from_millis(200));
            let current = fingerprint(&roots, &excluded)?;
            if current != previous {
                previous = current;
                break;
            }
        }
    }
}

fn watch_roots(plan: &BuildPlan, paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = vec![plan.project.workspace_root.clone()];
    roots.push(plan.config_path.clone());
    roots.extend(plan.config.build.inputs.iter().cloned());
    for resource in &plan.config.bundle.resources {
        let source = PathBuf::from(&resource.source);
        if source.exists() {
            roots.push(source);
        } else {
            let mut prefix = PathBuf::new();
            for component in source.components() {
                if matches!(component, std::path::Component::Normal(name) if name.to_string_lossy().contains(['*', '?', '[']))
                {
                    break;
                }
                prefix.push(component);
            }
            if !prefix.as_os_str().is_empty() {
                roots.push(prefix);
            }
        }
    }
    roots.extend(paths.iter().map(|p| plan.invocation_directory.join(p)));
    roots
}

fn launch(
    plan: &BuildPlan,
    args: &[String],
    changed: impl FnMut() -> Result<bool>,
) -> Result<(build::BuildRecord, bool)> {
    let record = build::compile(plan).context(report::stage("build", Some(plan)))?;
    let refreshed = crate::plan::resolve(&plan.selectors, true)?;
    let plan = &refreshed;
    let resources = resources::collect(&plan.config.bundle.resources)?;
    let development_resources = tempfile::tempdir_in(plan.root())?;
    resources::stage(&resources, development_resources.path())?;
    let development_binary = tempfile::tempdir_in(plan.root())?;
    let executable_root = if plan.os == "windows" {
        development_resources.path()
    } else {
        development_binary.path()
    };
    let executable = executable_root.join(
        record
            .snapshot_path
            .file_name()
            .context("snapshot filename")?,
    );
    ensure!(
        !executable.exists(),
        "development resource collides with executable"
    );
    std::fs::copy(&record.snapshot_path, &executable)?;
    ensure!(
        report::hash_file(&executable)? == record.executable_hash,
        "development executable copy changed"
    );
    let mut command = std::process::Command::new(executable);
    command.args(args).current_dir(&plan.project.directory);

    if !resources.is_empty() || plan.config.bundle.resource_contract.is_some() {
        command.env(
            "GUIC_RESOURCE_DIR",
            development_resources.path().canonicalize()?,
        );
    }
    let restarted = process::status_until(&mut command, changed)?;
    Ok((record, restarted))
}

fn fingerprint(
    roots: &[PathBuf],
    excluded: &[PathBuf],
) -> Result<BTreeMap<PathBuf, (u64, std::time::SystemTime)>> {
    let mut result = BTreeMap::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        let excluded = excluded.to_vec();
        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .require_git(false)
            .filter_entry(move |entry| {
                entry.file_name() != ".git"
                    && entry.file_name() != "target"
                    && !excluded.iter().any(|path| entry.path().starts_with(path))
            })
            .build();
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error)
                    if error
                        .io_error()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if entry.file_type().is_some_and(|kind| kind.is_file()) {
                let metadata = match entry.metadata() {
                    Ok(metadata) => metadata,
                    Err(error)
                        if error
                            .io_error()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                result.insert(entry.into_path(), (metadata.len(), metadata.modified()?));
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watcher_tracks_creation_deletion_and_ignores_outputs() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let roots = vec![dir.path().to_owned()];
        let excluded = vec![dir.path().join("custom-output")];
        std::fs::write(dir.path().join(".gitignore"), "generated/\n")?;
        let before = fingerprint(&roots, &excluded)?;
        for name in ["target", ".git", "generated", "custom-output"] {
            std::fs::create_dir(dir.path().join(name))?;
            std::fs::write(dir.path().join(name).join("ignored"), "output")?;
        }
        assert_eq!(fingerprint(&roots, &excluded)?, before);
        std::fs::write(dir.path().join("main.rs"), "source")?;
        assert_ne!(fingerprint(&roots, &excluded)?, before);
        std::fs::remove_file(dir.path().join("main.rs"))?;
        assert_eq!(fingerprint(&roots, &excluded)?, before);
        Ok(())
    }
}
