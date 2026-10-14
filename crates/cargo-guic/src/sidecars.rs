use crate::{config, plan::BuildPlan, report, resources::Input, staging};
use anyhow::{Result, ensure};
use std::{collections::BTreeSet, path::PathBuf};

pub(crate) fn collect(plan: &BuildPlan) -> Result<Vec<Input>> {
    let mut names = BTreeSet::new();
    let main = if plan.os == "windows" {
        format!("{}.exe", plan.project.bin)
    } else {
        plan.project.bin.clone()
    };
    staging::reserve(&mut names, &PathBuf::from(main))?;
    let mut result = Vec::new();
    for sidecar in &plan.config.bundle.sidecars {
        let source = PathBuf::from(sidecar.source.replace("{target}", &plan.target));
        config::reject_symlinks(&source)?;
        ensure!(source.is_file(), "sidecar is missing: {}", source.display());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                source.metadata()?.permissions().mode() & 0o111 != 0,
                "sidecar is not executable: {}",
                source.display()
            );
        }
        let destination = PathBuf::from(if plan.os == "windows" {
            format!("{}.exe", sidecar.name)
        } else {
            sidecar.name.clone()
        });
        staging::reserve(&mut names, &destination)?;
        if plan.os == "windows" {
            for resource in crate::resources::collect(&plan.config.bundle.resources)? {
                let mut collisions = BTreeSet::new();
                staging::reserve(&mut collisions, &destination)?;
                staging::reserve(&mut collisions, &resource.destination)?;
            }
        }
        result.push(Input {
            source: source.canonicalize()?,
            destination,
            sha256: report::hash_file(&source)?,
        });
    }
    Ok(result)
}

/// Copies immutable executable inputs using portable installed permissions.
pub(crate) fn stage(inputs: &[Input], root: &std::path::Path) -> Result<Vec<Input>> {
    let staged = crate::resources::stage(inputs, root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for input in &staged {
            std::fs::set_permissions(&input.source, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(staged)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidecar_inputs_are_hashed_and_collisions_are_rejected() -> Result<()> {
        let (directory, mut plan, _) = crate::build::fixture()?;
        let path = directory
            .path()
            .canonicalize()?
            .join(format!("helper-{}", plan.target));
        std::fs::write(&path, b"sidecar fixture")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
        plan.config.bundle.sidecars.push(config::Sidecar {
            name: "fixture-helper".into(),
            source: directory
                .path()
                .canonicalize()?
                .join("helper-{target}")
                .to_string_lossy()
                .into(),
        });
        let inputs = collect(&plan)?;
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].sha256, report::hash_file(&path)?);
        let staged = stage(&inputs, &directory.path().join("staged"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&staged[0].source)?.permissions().mode() & 0o777,
                0o755
            );
        }
        std::fs::write(path, b"modified")?;
        assert_eq!(std::fs::read(&staged[0].source)?, b"sidecar fixture");
        assert!(crate::resources::stage(&inputs, &directory.path().join("retry")).is_err());
        plan.config.bundle.sidecars[0].name = plan.project.bin.clone();
        assert!(collect(&plan).is_err());
        Ok(())
    }
}
