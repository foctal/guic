use crate::{
    config::{self, Resource},
    report, staging,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub sha256: String,
}

pub(crate) fn collect(resources: &[Resource]) -> Result<Vec<Input>> {
    let mut inputs = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    for resource in resources {
        staging::reserve(&mut BTreeSet::new(), &resource.destination)?;
        let source = Path::new(&resource.source);
        // Windows verbatim prefixes contain a literal question mark.
        let wildcard_component = |component: Component<'_>| match component {
            Component::Normal(name) => name.to_string_lossy().contains(['*', '?', '[']),
            _ => false,
        };
        // Existing paths are literal, including names containing glob metacharacters.
        let wildcard = !source.exists() && source.components().any(wildcard_component);
        let (base, files) = if wildcard {
            let mut base = PathBuf::new();
            for component in source.components() {
                if wildcard_component(component) {
                    break;
                }
                base.push(component);
            }
            let mut files = Vec::new();
            for path in glob::glob(&resource.source).context("invalid resource glob")? {
                let path = path?;
                config::reject_symlinks(&path)?;
                if path.is_file() {
                    files.push(path);
                }
            }
            (base, files)
        } else if source.is_dir() {
            config::reject_symlinks(source)?;
            let mut files = Vec::new();
            for entry in walkdir::WalkDir::new(source).follow_links(false) {
                let entry = entry?;
                ensure!(
                    !entry.file_type().is_symlink(),
                    "resource symlinks are not supported"
                );
                if entry.file_type().is_file() {
                    files.push(entry.into_path());
                } else {
                    ensure!(
                        entry.file_type().is_dir(),
                        "resources must contain regular files"
                    );
                }
            }
            (source.to_owned(), files)
        } else {
            (
                source.parent().context("source has no parent")?.to_owned(),
                vec![source.to_owned()],
            )
        };
        ensure!(
            !files.is_empty(),
            "resource source matched no files: {}",
            source.display()
        );
        for path in files {
            config::reject_symlinks(&path)?;
            ensure!(
                path.is_file(),
                "resource file is missing: {}",
                path.display()
            );
            let destination = if wildcard || source.is_dir() {
                resource.destination.join(path.strip_prefix(&base)?)
            } else {
                resource.destination.clone()
            };
            staging::reserve(&mut destinations, &destination)?;
            let input = Input {
                source: path.canonicalize()?,
                destination: destination.clone(),
                sha256: report::hash_file(&path)?,
            };
            inputs.insert(destination, input);
        }
    }
    Ok(inputs.into_values().collect())
}

pub(crate) fn stage(inputs: &[Input], root: &Path) -> Result<Vec<Input>> {
    let mut staged = Vec::new();
    for input in inputs {
        config::reject_symlinks(&input.source)?;
        let destination = root.join(&input.destination);
        std::fs::create_dir_all(destination.parent().context("resource parent")?)?;
        std::fs::copy(&input.source, &destination)?;
        ensure!(
            report::hash_file(&destination)? == input.sha256,
            "resource changed during staging"
        );
        staged.push(Input {
            source: destination,
            ..input.clone()
        });
    }
    Ok(staged)
}

// Framework symlinks are copied as links, only if their relative target stays inside
// the framework. Arbitrary resource inputs retain the stricter no-symlink policy.
pub(crate) fn copy_framework(source: &Path, destination: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(source)?);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_symlink() {
            let link = std::fs::read_link(entry.path())?;
            validate_link(source, entry.path(), &link)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, target)?;
            #[cfg(not(unix))]
            anyhow::bail!("framework symlinks require a native macOS host");
        } else {
            ensure!(
                entry.file_type().is_file(),
                "framework contains a non-regular file"
            );
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_link(root: &Path, path: &Path, link: &Path) -> Result<()> {
    ensure!(!link.is_absolute(), "absolute bundle symlink is forbidden");
    let mut depth = path
        .parent()
        .context("link parent")?
        .strip_prefix(root)?
        .components()
        .count();
    for component in link.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                ensure!(depth > 0, "bundle symlink escapes its root");
                depth -= 1;
            }
            _ => anyhow::bail!("invalid bundle symlink"),
        }
    }
    ensure!(
        path.canonicalize()?.starts_with(root.canonicalize()?),
        "bundle symlink resolves outside its root"
    );
    Ok(())
}

pub(crate) fn stage_backend(
    config: &crate::config::Packager,
    root: &Path,
) -> Result<(crate::config::Packager, BTreeMap<PathBuf, String>)> {
    crate::config::validate_backend_files(config)?;
    let mut config = config.clone();
    let mut hashes = BTreeMap::new();
    for (name, file) in [
        ("license", &mut config.license_file),
        ("info-plist", &mut config.macos_info_plist),
        ("entitlements", &mut config.macos_entitlements),
        ("dmg-background", &mut config.dmg_background),
        ("desktop-template", &mut config.deb_desktop_template),
        ("installer-icon", &mut config.nsis_installer_icon),
        ("header-image", &mut config.nsis_header_image),
        ("sidebar-image", &mut config.nsis_sidebar_image),
    ] {
        if let Some(source) = file {
            let destination =
                PathBuf::from(name).join(source.file_name().context("backend input filename")?);
            let hash = report::hash_file(source)?;
            let inputs = [Input {
                source: source.clone(),
                destination: destination.clone(),
                sha256: hash.clone(),
            }];
            let staged = stage(&inputs, root)?;
            *source = staged[0].source.clone();
            hashes.insert(PathBuf::from("backend").join(destination), hash);
        }
    }
    let mut names = BTreeSet::new();
    for source in &mut config.macos_frameworks {
        let name = source.file_name().context("framework filename")?;
        staging::reserve(&mut names, Path::new(name))?;
        let destination = root.join("frameworks").join(name);
        std::fs::create_dir_all(destination.parent().context("framework parent")?)?;
        let hash = crate::staging::hash_directory(source)?.1;
        copy_framework(source, &destination)?;
        ensure!(
            crate::staging::hash_directory(&destination)?.1 == hash,
            "framework changed during staging"
        );
        hashes.insert(PathBuf::from("backend/frameworks").join(name), hash);
        *source = destination;
    }
    crate::config::validate_backend_files(&config)?;
    Ok((config, hashes))
}

pub(crate) fn validate_installed_destinations(
    plan: &crate::plan::BuildPlan,
    inputs: &[Input],
) -> Result<()> {
    let mut reserved = BTreeSet::new();
    if plan.os == "windows" {
        for name in [format!("{}.exe", plan.project.bin), "uninstall.exe".into()] {
            staging::reserve(&mut reserved, Path::new(&name))?;
        }
    }
    if plan.os == "macos" && !plan.config.bundle.icons.is_empty() {
        let icon = plan
            .config
            .bundle
            .icons
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "icns"))
            .and_then(|p| p.file_name())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&plan.product_name).with_extension("icns"));
        staging::reserve(&mut reserved, &icon)?;
    }
    for input in inputs {
        staging::reserve(&mut reserved, &input.destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_globs_preserve_directories_and_reject_collisions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        std::fs::create_dir(root.join("data"))?;
        std::fs::write(root.join("data/b.txt"), "b")?;
        std::fs::write(root.join("data/a.txt"), "a")?;
        let resource = Resource {
            source: root.join("data/*.txt").to_string_lossy().into_owned(),
            destination: "data".into(),
        };
        let inputs = collect(std::slice::from_ref(&resource))?;
        assert_eq!(inputs[0].destination, Path::new("data/a.txt"));
        assert!(collect(&[resource.clone(), resource]).is_err());
        std::fs::write(&inputs[0].source, "changed")?;
        assert!(stage(&inputs, &root.join("stage")).is_err());
        Ok(())
    }
    #[test]
    fn literal_resource_names_with_glob_characters_are_preserved() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        std::fs::write(root.join("data[1].txt"), "literal resource")?;
        let inputs = collect(&[Resource {
            source: root.join("data[1].txt").to_string_lossy().into_owned(),
            destination: "data.txt".into(),
        }])?;
        assert_eq!(inputs.len(), 1);
        assert_eq!(std::fs::read(&inputs[0].source)?, b"literal resource");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn framework_links_are_preserved_but_escaping_links_are_rejected() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        let source = root.join("Example.framework");
        std::fs::create_dir_all(source.join("Versions/A"))?;
        std::fs::write(source.join("Versions/A/Example"), "library fixture")?;
        std::os::unix::fs::symlink("A", source.join("Versions/Current"))?;
        std::os::unix::fs::symlink("Versions/Current/Example", source.join("Example"))?;
        let destination = root.join("copy.framework");
        copy_framework(&source, &destination)?;
        assert_eq!(
            crate::staging::hash_directory(&source)?.1,
            crate::staging::hash_directory(&destination)?.1
        );
        std::fs::write(root.join("outside"), "outside")?;
        std::os::unix::fs::symlink("../outside", source.join("escape"))?;
        assert!(copy_framework(&source, &root.join("invalid.framework")).is_err());
        Ok(())
    }
}
