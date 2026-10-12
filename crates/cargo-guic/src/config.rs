use anyhow::{Context, Result, bail, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Config {
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    pub app: App,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub bundle: Bundle,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default)]
    pub signing: Signing,
    #[serde(default)]
    pub runtime: Runtime,
    #[serde(default)]
    pub backend: Backend,
    #[serde(default)]
    pub platform: BTreeMap<String, Override>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Override {
    pub app: Option<App>,
    pub build: Option<Build>,
    pub bundle: Option<Bundle>,
    pub hooks: Option<Hooks>,
    pub signing: Option<Signing>,
    pub runtime: Option<Runtime>,
    pub backend: Option<Backend>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct App {
    pub identifier: Option<String>,
    pub product_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Build {
    pub package: Option<String>,
    pub bin: Option<String>,
    pub target: Option<String>,
    pub profile: Option<String>,
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
    pub deployment_target: Option<String>,
    pub windows_resources: Option<guic_build::WindowsResources>,
    /// Files embedded or otherwise consumed by the build, including generated inputs.
    #[serde(default)]
    pub inputs: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Bundle {
    #[serde(default)]
    pub icons: Vec<PathBuf>,
    #[serde(default)]
    pub formats: Formats,
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Versioned installed resource layout implemented by the application.
    pub resource_contract: Option<ResourceContract>,
    #[serde(default)]
    pub file_associations: Vec<FileAssociation>,
    #[serde(default)]
    pub protocols: Vec<Protocol>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct FileAssociation {
    pub extensions: Vec<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Protocol {
    pub schemes: Vec<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Resource {
    pub source: String,
    pub destination: PathBuf,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ResourceContract {
    ApplicationManagedV1,
    GuicNativeV1,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Formats {
    #[serde(default)]
    pub macos: Vec<String>,
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub linux: Vec<String>,
}

impl Formats {
    pub fn for_os(&self, os: &str) -> Vec<String> {
        match os {
            "macos" => &self.macos,
            "windows" => &self.windows,
            _ => &self.linux,
        }
        .clone()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Hooks {
    pub before_build: Option<Hook>,
    pub before_bundle: Option<Hook>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Hook {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Policy {
    #[default]
    Off,
    Required,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Signing {
    #[serde(default)]
    pub code_signing: Policy,
    #[serde(default)]
    pub notarization: Policy,
    /// Public signing identity, never a key or certificate payload.
    pub macos_identity: Option<String>,
    /// Reference to credentials already stored with notarytool store-credentials.
    pub macos_keychain_profile: Option<String>,
    /// Certificate selector in the Windows certificate store.
    pub windows_thumbprint: Option<String>,
    pub windows_timestamp_url: Option<String>,
}

/// Explicit runtime policy; never inferred solely from dependency names.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Runtime {
    #[serde(default)]
    pub webview: bool,
    /// Maximum required GLIBC symbol version allowed in a Linux executable.
    pub glibc_max: Option<String>,
    /// Additional pkg-config build requirements.
    #[serde(default)]
    pub pkg_config: Vec<String>,
    /// Explicit dynamically loaded libraries not visible in the executable import table.
    #[serde(default)]
    pub dynamic_libraries: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Backend {
    #[serde(default)]
    pub cargo_packager: Packager,
}

// Only classified, non-executable, non-secret backend fields belong here.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Packager {
    pub publisher: Option<String>,
    pub copyright: Option<String>,
    pub long_description: Option<String>,
    pub category: Option<String>,
    pub license_file: Option<PathBuf>,
    pub macos_info_plist: Option<PathBuf>,
    pub macos_entitlements: Option<PathBuf>,
    #[serde(default)]
    pub macos_frameworks: Vec<PathBuf>,
    pub dmg_background: Option<PathBuf>,
    #[serde(default)]
    pub deb_depends: Vec<String>,
    pub deb_desktop_template: Option<PathBuf>,
    pub nsis_installer_icon: Option<PathBuf>,
    pub nsis_header_image: Option<PathBuf>,
    pub nsis_sidebar_image: Option<PathBuf>,
}

pub(crate) struct Loaded {
    pub config: Config,
    pub provenance: BTreeMap<String, String>,
}

pub(crate) fn load(path: &Path, os: &str) -> Result<Loaded> {
    let input = std::fs::read_to_string(path).with_context(|| {
        format!(
            "read {}; run cargo guic init --identifier <reverse-domain-id>",
            path.display()
        )
    })?;
    // Avoid echoing source lines: they may contain rejected credentials.
    let mut value: toml::Value =
        toml::from_str(&input).map_err(|_| anyhow::anyhow!("invalid Guic.toml syntax"))?;
    let original: Config = value.clone().try_into().map_err(|_| anyhow::anyhow!("Guic.toml contains unknown keys or invalid field types; see cargo guic schema (credentials and backend hooks are forbidden)"))?;
    ensure!(
        original.schema_version == 1,
        "unsupported configuration schema; migrate Guic.toml to schema-version = 1"
    );
    for name in original.platform.keys() {
        ensure!(
            ["macos", "windows", "linux"].contains(&name.as_str()),
            "unknown platform table"
        );
    }
    let table = value
        .as_table_mut()
        .context("configuration must be a table")?;
    let overrides = table.remove("platform");
    let mut provenance = BTreeMap::new();
    provenance_for(&value, "", "config", &mut provenance);
    if let Some(overlay) = overrides.as_ref().and_then(|v| v.get(os)) {
        merge(&mut value, overlay.clone());
        provenance_for(overlay, "", &format!("platform.{os}"), &mut provenance);
    }
    let mut config: Config = value.try_into().context("invalid resolved configuration")?;
    validate_identifier(
        config
            .app
            .identifier
            .as_deref()
            .context("app.identifier is required")?,
    )?;
    if let Some(name) = &config.app.product_name {
        validate_name(name)?;
    }
    ensure!(
        config.signing.notarization != Policy::Required
            || config.signing.code_signing == Policy::Required,
        "required notarization requires required code signing"
    );
    for name in config
        .runtime
        .pkg_config
        .iter()
        .chain(&config.runtime.dynamic_libraries)
    {
        ensure!(
            !name.is_empty()
                && !name.starts_with('-')
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b)),
            "runtime requirements must be plain package or library names"
        );
    }
    if let Some(version) = &config.runtime.glibc_max {
        ensure!(
            version.split('.').count() >= 2 && version.split('.').all(|p| p.parse::<u32>().is_ok()),
            "runtime.glibc-max must be a numeric version"
        );
    }
    let base = path.parent().context("configuration has no parent")?;
    ensure!(
        config.bundle.resources.is_empty() || config.bundle.resource_contract.is_some(),
        "external resources require bundle.resource-contract = 'application-managed-v1'; or guic-native-v1 with guic_assets::ResourceRoot"
    );
    for resource in &mut config.bundle.resources {
        let source = base.canonicalize()?.join(&resource.source);
        resource.source = source
            .to_str()
            .context("resource source path must be UTF-8")?
            .into();
    }
    for association in &config.bundle.file_associations {
        ensure!(
            !association.extensions.is_empty()
                && association.extensions.iter().all(|ext| !ext.is_empty()
                    && ext
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')),
            "file association extensions must be nonempty extensions without leading dots"
        );
    }
    for protocol in &config.bundle.protocols {
        ensure!(
            !protocol.schemes.is_empty()
                && protocol.schemes.iter().all(|scheme| scheme
                    .starts_with(|c: char| c.is_ascii_alphabetic())
                    && scheme
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))),
            "invalid URL protocol scheme"
        );
    }
    let backend = &mut config.backend.cargo_packager;
    for path in [
        &mut backend.license_file,
        &mut backend.macos_info_plist,
        &mut backend.macos_entitlements,
        &mut backend.dmg_background,
        &mut backend.deb_desktop_template,
        &mut backend.nsis_installer_icon,
        &mut backend.nsis_header_image,
        &mut backend.nsis_sidebar_image,
    ]
    .into_iter()
    .flatten()
    {
        *path = file(base, path)?;
    }
    validate_backend_files(backend)?;
    for path in &mut backend.macos_frameworks {
        *path = base
            .join(&*path)
            .canonicalize()
            .context("framework must exist")?;
        ensure!(
            path.is_dir() && path.extension().is_some_and(|ext| ext == "framework"),
            "macos-frameworks requires framework directories"
        );
    }
    if let Some(category) = &backend.category {
        let _: cargo_packager::config::AppCategory = category
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid backend category"))?;
    }
    if let Some(windows) = &mut config.build.windows_resources {
        ensure!(
            !windows.use_dependency_manifest || windows.manifest.is_none(),
            "use-dependency-manifest cannot be combined with a custom manifest"
        );
        for path in [&mut windows.icon, &mut windows.manifest]
            .into_iter()
            .flatten()
        {
            *path = file(base, path)?;
        }
        if let Some(icon) = &windows.icon {
            ensure!(
                icon.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("ico")),
                "Windows executable icon must be an ICO file"
            );
        }
    }
    for input in &mut config.build.inputs {
        *input = base.canonicalize()?.join(&*input);
        reject_symlinks(input)?;
        ensure!(
            input.is_file() || (config.hooks.before_build.is_some() && !input.exists()),
            "build input is missing or is not a regular file: {}",
            input.display()
        );
    }
    for icon in &mut config.bundle.icons {
        *icon = file(base, icon)?;
    }
    for hook in [
        &mut config.hooks.before_build,
        &mut config.hooks.before_bundle,
    ]
    .into_iter()
    .flatten()
    {
        ensure!(
            !hook.program.trim().is_empty(),
            "hook program must not be empty"
        );
        hook.cwd = base
            .join(&hook.cwd)
            .canonicalize()
            .context("hook cwd must exist")?;
        ensure!(hook.cwd.is_dir(), "hook cwd must be a directory");
        if Path::new(&hook.program).components().count() > 1 {
            hook.program = file(base, Path::new(&hook.program))?
                .to_string_lossy()
                .into_owned();
        }
    }
    Ok(Loaded { config, provenance })
}

pub(crate) fn validate_backend_files(backend: &Packager) -> Result<()> {
    if let Some(path) = &backend.macos_info_plist {
        let value = plist::Value::from_file(path).context("invalid macOS Info.plist")?;
        let dictionary = value
            .as_dictionary()
            .context("Info.plist must be a dictionary")?;
        for key in [
            "CFBundleExecutable",
            "CFBundleIdentifier",
            "CFBundleVersion",
            "CFBundleShortVersionString",
            "CFBundleName",
            "CFBundleDisplayName",
            "LSMinimumSystemVersion",
        ] {
            ensure!(
                !dictionary.contains_key(key),
                "Info.plist key {key} is owned by cargo-guic; configure the corresponding app/build setting"
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_identifier(id: &str) -> Result<()> {
    ensure!(
        id.len() <= 255
            && id.split('.').count() >= 2
            && id.split('.').all(|p| !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !p.starts_with('-')
                && !p.ends_with('-')),
        "app.identifier must be a stable reverse-domain identifier, such as com.example.app"
    );
    Ok(())
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && !name.chars().any(|c| c.is_control() || "/\\:".contains(c))
            && name != "."
            && name != "..",
        "product-name must be a nonempty safe display name"
    );
    Ok(())
}

pub(crate) fn file(base: &Path, path: &Path) -> Result<PathBuf> {
    let path = base.join(path);
    reject_symlinks(&path)?;
    let path = path
        .canonicalize()
        .with_context(|| format!("required file is missing: {}", path.display()))?;
    ensure!(
        path.is_file(),
        "expected a regular file: {}",
        path.display()
    );
    Ok(path)
}

pub(crate) fn reject_symlinks(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
            bail!(
                "symbolic links are not supported in managed inputs: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn provenance_for(
    value: &toml::Value,
    prefix: &str,
    source: &str,
    out: &mut BTreeMap<String, String>,
) {
    if let Some(table) = value.as_table() {
        for (key, value) in table {
            let field = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            provenance_for(value, &field, source, out);
        }
    } else {
        out.insert(prefix.into(), source.into());
    }
}

fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(existing) = base.get_mut(&key) {
                    merge(existing, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, value) => *base = value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_tables_and_replaces_arrays() -> Result<()> {
        let mut base: toml::Value = toml::from_str("[build]\nfeatures=['old']\npackage='app'")?;
        merge(&mut base, toml::from_str("[build]\nfeatures=['new']")?);
        assert_eq!(base["build"]["package"].as_str(), Some("app"));
        assert_eq!(base["build"]["features"].as_array().map(Vec::len), Some(1));
        assert_eq!(base["build"]["features"][0].as_str(), Some("new"));
        Ok(())
    }
    #[test]
    fn identifiers_are_explicit_and_safe() {
        for id in ["", "app", "a..b", "../app", "a.-b", "a.b/"] {
            assert!(validate_identifier(id).is_err());
        }
        assert!(validate_identifier("org.example.my-app").is_ok());
    }
}
