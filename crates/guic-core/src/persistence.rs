use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Identifies the file used to satisfy a recovering load.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadSource {
    /// The primary JSON file was valid.
    Primary,
    /// The primary file was missing or invalid and the backup was valid.
    Backup,
}

/// A value loaded with its recovery source.
#[derive(Debug, Eq, PartialEq)]
pub struct Recovered<T> {
    /// The deserialized value.
    pub value: T,
    /// The file from which the value was loaded.
    pub source: LoadSource,
}

/// Errors produced by [`JsonStore`].
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// A filesystem operation failed.
    #[error("failed to {operation} `{path}`: {source}")]
    Io {
        /// The operation being performed.
        operation: &'static str,
        /// The affected path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON serialization failed.
    #[error("failed to serialize JSON for `{path}`: {source}")]
    Serialize {
        /// The destination path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: serde_json::Error,
    },
    /// JSON deserialization failed.
    #[error("failed to deserialize JSON from `{path}`: {source}")]
    Deserialize {
        /// The source path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: serde_json::Error,
    },
    /// Neither the primary file nor its backup could be loaded.
    #[error("primary load failed: {primary}; backup load failed: {backup}")]
    Recovery {
        /// The primary-file failure.
        primary: Box<Self>,
        /// The backup-file failure.
        backup: Box<Self>,
    },
}

/// A crash-resistant JSON file store for application state.
///
/// Saves are written and synchronized in the destination directory before the
/// previous value is moved to a `.bak` file and replaced. If replacement is
/// interrupted, [`JsonStore::load_recovering`] can read the backup. A store is
/// intended to have one writer; applications should serialize concurrent save
/// requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonStore {
    path: PathBuf,
}

impl JsonStore {
    /// Creates a store for `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the primary JSON path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the adjacent backup path used by this store.
    #[must_use]
    pub fn backup_path(&self) -> PathBuf {
        sibling_with_suffix(&self.path, ".bak")
    }

    /// Loads the primary file, returning `None` when it does not exist.
    pub fn load<T: DeserializeOwned>(&self) -> Result<Option<T>, PersistenceError> {
        match File::open(&self.path) {
            Ok(file) => deserialize(file, &self.path).map(Some),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(io_error("open", &self.path, source)),
        }
    }

    /// Loads the primary file or returns `T::default()` when it does not exist.
    pub fn load_or_default<T: DeserializeOwned + Default>(&self) -> Result<T, PersistenceError> {
        Ok(self.load()?.unwrap_or_default())
    }

    /// Loads the primary file and falls back to the adjacent backup on failure.
    ///
    /// Returns `Ok(None)` only when neither file exists. If either file exists
    /// but neither is valid, both failures are preserved in
    /// [`PersistenceError::Recovery`].
    pub fn load_recovering<T: DeserializeOwned>(
        &self,
    ) -> Result<Option<Recovered<T>>, PersistenceError> {
        match self.load() {
            Ok(Some(value)) => Ok(Some(Recovered {
                value,
                source: LoadSource::Primary,
            })),
            Ok(None) => self.load_backup(None),
            Err(primary) => self.load_backup(Some(primary)),
        }
    }

    /// Serializes and replaces the stored value while preserving one backup.
    pub fn save<T: Serialize>(&self, value: &T) -> Result<(), PersistenceError> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create directory", parent, source))?;

        let temporary = temporary_path(&self.path);
        let result = self.write_temporary(&temporary, value);
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        let backup = self.backup_path();
        if self.path.exists() {
            if backup.exists() {
                fs::remove_file(&backup)
                    .map_err(|source| io_error("remove old backup", &backup, source))?;
            }
            fs::rename(&self.path, &backup)
                .map_err(|source| io_error("create backup", &self.path, source))?;
        }

        if let Err(source) = fs::rename(&temporary, &self.path) {
            if backup.exists() && !self.path.exists() {
                let _ = fs::rename(&backup, &self.path);
            }
            let _ = fs::remove_file(&temporary);
            return Err(io_error("replace", &self.path, source));
        }

        sync_directory(parent);
        Ok(())
    }

    fn write_temporary<T: Serialize>(
        &self,
        temporary: &Path,
        value: &T,
    ) -> Result<(), PersistenceError> {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary)
            .map_err(|source| io_error("create temporary file", temporary, source))?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, value).map_err(|source| {
            PersistenceError::Serialize {
                path: self.path.clone(),
                source,
            }
        })?;
        writer
            .write_all(b"\n")
            .map_err(|source| io_error("write temporary file", temporary, source))?;
        writer
            .flush()
            .map_err(|source| io_error("flush temporary file", temporary, source))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|source| io_error("synchronize temporary file", temporary, source))
    }

    fn load_backup<T: DeserializeOwned>(
        &self,
        primary: Option<PersistenceError>,
    ) -> Result<Option<Recovered<T>>, PersistenceError> {
        let backup = self.backup_path();
        match File::open(&backup) {
            Ok(file) => match deserialize(file, &backup) {
                Ok(value) => Ok(Some(Recovered {
                    value,
                    source: LoadSource::Backup,
                })),
                Err(backup_error) => Err(match primary {
                    Some(primary) => PersistenceError::Recovery {
                        primary: Box::new(primary),
                        backup: Box::new(backup_error),
                    },
                    None => backup_error,
                }),
            },
            Err(source) if source.kind() == std::io::ErrorKind::NotFound && primary.is_none() => {
                Ok(None)
            }
            Err(source) => {
                let backup_error = io_error("open", &backup, source);
                Err(match primary {
                    Some(primary) => PersistenceError::Recovery {
                        primary: Box::new(primary),
                        backup: Box::new(backup_error),
                    },
                    None => backup_error,
                })
            }
        }
    }
}

fn deserialize<T: DeserializeOwned>(file: File, path: &Path) -> Result<T, PersistenceError> {
    serde_json::from_reader(BufReader::new(file)).map_err(|source| PersistenceError::Deserialize {
        path: path.to_path_buf(),
        source,
    })
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> PersistenceError {
    PersistenceError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    sibling_with_suffix(path, &format!(".tmp-{}-{sequence}", std::process::id()))
}

#[cfg(unix)]
fn sync_directory(path: &Path) {
    if let Ok(directory) = File::open(path) {
        let _ = directory.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) {}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    use super::{JsonStore, LoadSource, PersistenceError};

    #[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
    struct Settings {
        theme: String,
        recent: Vec<String>,
    }

    #[test]
    fn missing_store_loads_none_or_default() {
        let directory = tempdir().expect("temporary directory should be created");
        let store = JsonStore::new(directory.path().join("settings.json"));

        assert_eq!(store.load::<Settings>().expect("load should succeed"), None);
        assert_eq!(
            store
                .load_or_default::<Settings>()
                .expect("default load should succeed"),
            Settings::default()
        );
    }

    #[test]
    fn save_round_trips_and_preserves_previous_value() {
        let directory = tempdir().expect("temporary directory should be created");
        let store = JsonStore::new(directory.path().join("state/settings.json"));
        let first = Settings {
            theme: "light".into(),
            recent: vec!["one".into()],
        };
        let second = Settings {
            theme: "dark".into(),
            recent: vec!["two".into()],
        };

        store.save(&first).expect("first save should succeed");
        store.save(&second).expect("second save should succeed");

        assert_eq!(
            store.load::<Settings>().expect("load should succeed"),
            Some(second)
        );
        let backup = JsonStore::new(store.backup_path());
        assert_eq!(
            backup.load::<Settings>().expect("backup should load"),
            Some(first)
        );
    }

    #[test]
    fn recovering_load_uses_backup_after_primary_corruption() {
        let directory = tempdir().expect("temporary directory should be created");
        let store = JsonStore::new(directory.path().join("settings.json"));
        let first = Settings {
            theme: "light".into(),
            recent: Vec::new(),
        };
        let second = Settings {
            theme: "dark".into(),
            recent: Vec::new(),
        };
        store.save(&first).expect("first save should succeed");
        store.save(&second).expect("second save should succeed");
        std::fs::write(store.path(), "{broken").expect("corruption should succeed");

        let recovered = store
            .load_recovering::<Settings>()
            .expect("recovery should succeed")
            .expect("a value should be recovered");
        assert_eq!(recovered.source, LoadSource::Backup);
        assert_eq!(recovered.value, first);
    }

    #[test]
    fn recovery_preserves_both_parse_failures() {
        let directory = tempdir().expect("temporary directory should be created");
        let store = JsonStore::new(directory.path().join("settings.json"));
        std::fs::write(store.path(), "{broken").expect("primary write should succeed");
        std::fs::write(store.backup_path(), "{also-broken").expect("backup write should succeed");

        assert!(matches!(
            store.load_recovering::<Settings>(),
            Err(PersistenceError::Recovery { .. })
        ));
    }
}
