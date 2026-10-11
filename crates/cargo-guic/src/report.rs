use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) fn print(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    writeln!(stdout)?;
    Ok(())
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn hash_json(value: &impl Serialize) -> Result<String> {
    // Value uses sorted map keys, making map iteration order irrelevant.
    Ok(digest(&serde_json::to_vec(&serde_json::to_value(value)?)?))
}
pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn atomic_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("record has no parent")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    crate::process::check_cancelled()?;
    file.persist_noclobber(path)
        .context("refusing to replace an existing record")?;
    Ok(())
}

pub(crate) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let value: serde_json::Value = serde_json::from_reader(File::open(path)?)?;
    ensure!(
        value.get("schema_version").and_then(|v| v.as_u64()) == Some(1),
        "unsupported record/worker schema; regenerate with this cargo-guic version"
    );
    serde_json::from_value(value).context("invalid record or worker message")
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactReport {
    pub schema_version: u32,
    pub identifier: String,
    pub version: String,
    pub build_record_id: String,
    pub target: String,
    pub profile: String,
    pub features: Vec<String>,
    pub default_features: bool,
    pub requested_formats: Vec<String>,
    pub resource_hashes: BTreeMap<PathBuf, String>,
    pub tool_versions: BTreeMap<String, String>,
    pub outputs: Vec<Artifact>,
    pub code_signing: String,
    pub notarization: String,
    pub stapling: String,
    pub complete: bool,
    pub failed_format: Option<String>,
    #[serde(default)]
    pub trial: bool,
    #[serde(default)]
    pub acceptance: String,
    #[serde(default)]
    pub cli_executable_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Artifact {
    pub format: String,
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub kind: String,
    #[serde(default)]
    pub verification: crate::signing::Verification,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorContext {
    pub stage: &'static str,
    pub application: Option<String>,
    pub target: Option<String>,
    pub formats: Vec<String>,
}
impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} stage", self.stage)?;
        if let Some(app) = &self.application {
            write!(f, ": {app}")?;
        }
        if let Some(target) = &self.target {
            write!(f, " ({target})")?;
        }
        Ok(())
    }
}
impl std::error::Error for ErrorContext {}
pub(crate) fn stage(stage: &'static str, plan: Option<&crate::plan::BuildPlan>) -> ErrorContext {
    ErrorContext {
        stage,
        application: plan.map(|p| p.identifier.clone()),
        target: plan.map(|p| p.target.clone()),
        formats: plan.map(|p| p.formats.clone()).unwrap_or_default(),
    }
}
