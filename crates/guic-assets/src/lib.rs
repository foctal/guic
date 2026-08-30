//! Asset loading abstractions for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use gpui::{App, AssetSource, Global, SharedString};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// A stable asset key.
pub type AssetKey = String;

/// Supported asset kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    /// Raster image assets.
    Image,
    /// SVG or vector-like assets.
    Vector,
    /// Font assets.
    Font,
    /// Arbitrary application data.
    Data,
}

impl AssetKind {
    /// Infers an asset kind from a file path.
    ///
    /// # Example
    ///
    /// ```
    /// use guic_assets::AssetKind;
    ///
    /// assert_eq!(AssetKind::infer_from_path("logo.svg"), AssetKind::Vector);
    /// assert_eq!(AssetKind::infer_from_path("theme.json"), AssetKind::Data);
    /// ```
    #[must_use]
    pub fn infer_from_path(path: impl AsRef<Path>) -> Self {
        match path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp") => Self::Image,
            Some("svg") => Self::Vector,
            Some("ttf" | "otf" | "woff" | "woff2") => Self::Font,
            _ => Self::Data,
        }
    }
}

/// A registered asset descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssetSpec {
    /// Stable asset key.
    pub key: AssetKey,
    /// Asset kind.
    pub kind: AssetKind,
    /// Relative or absolute source path.
    pub path: String,
}

impl AssetSpec {
    /// Creates a new asset descriptor.
    #[must_use]
    pub fn new(key: impl Into<AssetKey>, kind: AssetKind, path: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            kind,
            path: path.into(),
        }
    }
}

/// Asset loading errors.
#[derive(Debug, Error)]
pub enum AssetError {
    /// The requested asset key was not registered.
    #[error("asset key `{0}` is not registered")]
    MissingKey(String),
    /// An I/O failure occurred while loading an asset.
    #[error("failed to load asset `{path}`: {source}")]
    Io {
        /// Asset path that failed.
        path: String,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// Asset contents were not valid UTF-8.
    #[error("asset `{0}` is not valid UTF-8")]
    InvalidUtf8(String),
    /// An asset manifest document could not be parsed.
    #[error("failed to parse asset manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// A serializable asset manifest document for packaged registration flows.
///
/// # Example
///
/// ```
/// use guic_assets::{AssetKind, AssetManifestDocument, AssetSpec};
///
/// let document = AssetManifestDocument::new(vec![
///     AssetSpec::new("logo", AssetKind::Vector, "assets/logo.svg"),
/// ]);
/// assert_eq!(document.assets.len(), 1);
/// ```
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssetManifestDocument {
    /// Asset descriptors to register.
    pub assets: Vec<AssetSpec>,
}

impl AssetManifestDocument {
    /// Creates a new manifest document.
    #[must_use]
    pub fn new(assets: Vec<AssetSpec>) -> Self {
        Self { assets }
    }

    /// Parses an asset manifest document from JSON.
    pub fn from_json_str(json: &str) -> Result<Self, AssetError> {
        Ok(serde_json::from_str(json)?)
    }
}

/// A filesystem-backed GPUI asset source.
///
/// # Example
///
/// ```no_run
/// use guic_assets::FileAssetSource;
///
/// let source = FileAssetSource::new("assets");
/// assert!(source.base().ends_with("assets"));
/// ```
#[derive(Clone, Debug)]
pub struct FileAssetSource {
    base: PathBuf,
}

impl Default for FileAssetSource {
    fn default() -> Self {
        Self {
            base: PathBuf::from("/"),
        }
    }
}

impl FileAssetSource {
    /// Creates a new file asset source rooted at the given directory.
    #[must_use]
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    /// Returns the base directory.
    #[must_use]
    pub fn base(&self) -> &Path {
        &self.base
    }

    fn resolve(&self, path: &str) -> PathBuf {
        let candidate = PathBuf::from(path);
        if candidate.is_absolute() {
            candidate
        } else {
            self.base.join(candidate)
        }
    }
}

impl AssetSource for FileAssetSource {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let resolved = self.resolve(path);
        match fs::read(&resolved) {
            Ok(bytes) => Ok(Some(Cow::Owned(bytes))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let resolved = self.resolve(path);
        let entries = fs::read_dir(resolved)?
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|entry| entry.file_name().into_string().ok())
                    .map(SharedString::from)
            })
            .collect();
        Ok(entries)
    }
}

/// A global asset manifest.
///
/// # Example
///
/// ```no_run
/// use guic_assets::{AssetKind, AssetManifest, AssetSpec, FileAssetSource};
///
/// let mut manifest = AssetManifest::default();
/// manifest.register(AssetSpec::new("logo", AssetKind::Vector, "assets/logo.svg"));
///
/// let source = FileAssetSource::new(".");
/// let bytes = manifest.load_bytes(&source, "logo")?;
/// # Ok::<(), guic_assets::AssetError>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct AssetManifest {
    assets: BTreeMap<AssetKey, AssetSpec>,
}

impl Global for AssetManifest {}

impl AssetManifest {
    /// Returns the global manifest.
    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Returns the global manifest mutably.
    #[must_use]
    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Registers an asset spec, replacing an existing one with the same key.
    pub fn register(&mut self, spec: AssetSpec) {
        self.assets.insert(spec.key.clone(), spec);
    }

    /// Registers all assets from a manifest document.
    pub fn register_document(&mut self, document: AssetManifestDocument) -> usize {
        let count = document.assets.len();
        for spec in document.assets {
            self.register(spec);
        }
        count
    }

    /// Registers assets from a manifest JSON string.
    pub fn register_manifest_json(&mut self, json: &str) -> Result<usize, AssetError> {
        Ok(self.register_document(AssetManifestDocument::from_json_str(json)?))
    }

    /// Registers assets from a manifest JSON file.
    pub fn register_manifest_file(&mut self, path: impl AsRef<Path>) -> Result<usize, AssetError> {
        let path = path.as_ref();
        let json = fs::read_to_string(path).map_err(|source| AssetError::Io {
            path: path.display().to_string(),
            source,
        })?;
        self.register_manifest_json(&json)
    }

    /// Looks up an asset by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&AssetSpec> {
        self.assets.get(key)
    }

    /// Returns the registered asset count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Returns whether the manifest is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Loads the bytes for a registered asset using the provided filesystem source.
    pub fn load_bytes(&self, source: &FileAssetSource, key: &str) -> Result<Vec<u8>, AssetError> {
        let spec = self
            .get(key)
            .ok_or_else(|| AssetError::MissingKey(key.to_owned()))?;
        let path = source.resolve(&spec.path);
        fs::read(&path).map_err(|source| AssetError::Io {
            path: path.display().to_string(),
            source,
        })
    }

    /// Loads a registered text asset as UTF-8.
    pub fn load_string(&self, source: &FileAssetSource, key: &str) -> Result<String, AssetError> {
        let bytes = self.load_bytes(source, key)?;
        String::from_utf8(bytes).map_err(|_| AssetError::InvalidUtf8(key.to_owned()))
    }

    /// Registers every file in the given directory under `prefix/name`.
    pub fn register_directory(
        &mut self,
        prefix: &str,
        kind: AssetKind,
        dir: impl AsRef<Path>,
    ) -> Result<usize, AssetError> {
        let mut count = 0usize;
        for entry in fs::read_dir(dir.as_ref()).map_err(|source| AssetError::Io {
            path: dir.as_ref().display().to_string(),
            source,
        })? {
            let entry = entry.map_err(|source| AssetError::Io {
                path: dir.as_ref().display().to_string(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| AssetError::Io {
                    path: entry.path().display().to_string(),
                    source,
                })?
                .is_file()
            {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let key = format!("{prefix}/{file_name}");
            self.register(AssetSpec::new(
                key,
                kind,
                entry.path().display().to_string(),
            ));
            count += 1;
        }
        Ok(count)
    }

    /// Registers every file in the given directory tree under `prefix/relative/path`.
    pub fn register_directory_recursive(
        &mut self,
        prefix: &str,
        kind: AssetKind,
        dir: impl AsRef<Path>,
    ) -> Result<usize, AssetError> {
        self.register_directory_tree(prefix, dir.as_ref(), Some(kind))
    }

    /// Registers every file in the given directory tree, inferring asset kinds from file names.
    pub fn register_directory_inferred(
        &mut self,
        prefix: &str,
        dir: impl AsRef<Path>,
    ) -> Result<usize, AssetError> {
        self.register_directory_tree(prefix, dir.as_ref(), None)
    }

    fn register_directory_tree(
        &mut self,
        prefix: &str,
        dir: &Path,
        kind_override: Option<AssetKind>,
    ) -> Result<usize, AssetError> {
        let mut count = 0usize;
        let mut stack = vec![dir.to_path_buf()];

        while let Some(current) = stack.pop() {
            for entry in fs::read_dir(&current).map_err(|source| AssetError::Io {
                path: current.display().to_string(),
                source,
            })? {
                let entry = entry.map_err(|source| AssetError::Io {
                    path: current.display().to_string(),
                    source,
                })?;
                let entry_path = entry.path();
                let file_type = entry.file_type().map_err(|source| AssetError::Io {
                    path: entry_path.display().to_string(),
                    source,
                })?;

                if file_type.is_dir() {
                    stack.push(entry_path);
                    continue;
                }

                if !file_type.is_file() {
                    continue;
                }

                let relative = entry_path
                    .strip_prefix(dir)
                    .expect("directory walk should stay within root");
                let relative = relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/");
                let key = if prefix.is_empty() {
                    relative.clone()
                } else {
                    format!("{prefix}/{relative}")
                };
                let kind = kind_override.unwrap_or_else(|| AssetKind::infer_from_path(&entry_path));
                self.register(AssetSpec::new(key, kind, entry_path.display().to_string()));
                count += 1;
            }
        }

        Ok(count)
    }
}

/// Initializes the asset manifest global.
pub fn init(cx: &mut App) {
    if !cx.has_global::<AssetManifest>() {
        cx.set_global(AssetManifest::default());
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetKind, AssetManifest, AssetManifestDocument, AssetSpec, FileAssetSource};
    use gpui::AssetSource as _;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should advance")
            .as_nanos();
        std::env::temp_dir().join(format!("guic-assets-{name}-{stamp}"))
    }

    #[test]
    fn stores_assets_by_key() {
        let mut manifest = AssetManifest::default();
        manifest.register(AssetSpec::new("logo", AssetKind::Vector, "assets/logo.svg"));

        let logo = manifest.get("logo").expect("asset should be registered");
        assert_eq!(logo.kind, AssetKind::Vector);
        assert_eq!(logo.path, "assets/logo.svg");
    }

    #[test]
    fn file_asset_source_loads_relative_paths() {
        let root = temp_dir("load");
        fs::create_dir_all(&root).expect("temp dir should be created");
        let file = root.join("theme.json");
        fs::write(&file, br#"{"name":"test"}"#).expect("test file should be written");

        let source = FileAssetSource::new(&root);
        let bytes = source
            .load("theme.json")
            .expect("load should succeed")
            .expect("asset should exist");
        assert_eq!(bytes.as_ref(), br#"{"name":"test"}"#);

        fs::remove_dir_all(root).expect("temp dir should be removed");
    }

    #[test]
    fn manifest_loads_registered_text_assets() {
        let root = temp_dir("manifest");
        fs::create_dir_all(&root).expect("temp dir should be created");
        let file = root.join("palette.txt");
        fs::write(&file, "ok").expect("test file should be written");

        let mut manifest = AssetManifest::default();
        manifest.register(AssetSpec::new(
            "palette",
            AssetKind::Data,
            file.display().to_string(),
        ));

        let source = FileAssetSource::default();
        let loaded = manifest
            .load_string(&source, "palette")
            .expect("string asset should load");
        assert_eq!(loaded, "ok");

        fs::remove_dir_all(root).expect("temp dir should be removed");
    }

    #[test]
    fn registers_assets_from_manifest_json() {
        let mut manifest = AssetManifest::default();
        let count = manifest
            .register_manifest_json(
                r#"{"assets":[{"key":"logo","kind":"vector","path":"assets/logo.svg"}]}"#,
            )
            .expect("manifest JSON should parse");

        assert_eq!(count, 1);
        assert_eq!(
            manifest.get("logo").map(|asset| asset.kind),
            Some(AssetKind::Vector)
        );
    }

    #[test]
    fn registers_assets_from_recursive_directory_with_inferred_kinds() {
        let root = temp_dir("recursive");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        fs::write(root.join("logo.svg"), "<svg/>").expect("svg should be written");
        fs::write(nested.join("font.woff2"), "font").expect("font should be written");
        fs::write(nested.join("config.json"), "{}").expect("data should be written");

        let mut manifest = AssetManifest::default();
        let count = manifest
            .register_directory_inferred("bundle", &root)
            .expect("directory tree should register");

        assert_eq!(count, 3);
        assert_eq!(
            manifest.get("bundle/logo.svg").map(|asset| asset.kind),
            Some(AssetKind::Vector)
        );
        assert_eq!(
            manifest
                .get("bundle/nested/font.woff2")
                .map(|asset| asset.kind),
            Some(AssetKind::Font)
        );
        assert_eq!(
            manifest
                .get("bundle/nested/config.json")
                .map(|asset| asset.kind),
            Some(AssetKind::Data)
        );

        fs::remove_dir_all(root).expect("temp dir should be removed");
    }

    #[test]
    fn asset_kind_is_inferred_from_extensions() {
        assert_eq!(AssetKind::infer_from_path("image.png"), AssetKind::Image);
        assert_eq!(AssetKind::infer_from_path("icon.svg"), AssetKind::Vector);
        assert_eq!(AssetKind::infer_from_path("font.ttf"), AssetKind::Font);
        assert_eq!(AssetKind::infer_from_path("notes.md"), AssetKind::Data);
    }

    #[test]
    fn manifest_document_roundtrips() {
        let document = AssetManifestDocument::new(vec![AssetSpec::new(
            "logo",
            AssetKind::Vector,
            "assets/logo.svg",
        )]);
        let json = serde_json::to_string(&document).expect("manifest should serialize");
        let parsed = AssetManifestDocument::from_json_str(&json).expect("manifest should parse");
        assert_eq!(parsed, document);
    }
}
