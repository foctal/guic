mod cargo_packager;

use crate::{plan::BuildPlan, process, report};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) const VERSION: &str = "0.11.8";

pub(crate) fn validate(plan: &BuildPlan, execute_signing: bool) -> Result<()> {
    crate::signing::validate(&plan.config.signing, &plan.os, execute_signing)?;
    ensure!(
        !plan.formats.is_empty(),
        "no package formats configured; configure bundle.formats, pass --formats, or use build --no-bundle"
    );
    if plan.target == "universal-apple-darwin" {
        ensure!(
            crate::platform::find("lipo").is_some(),
            "Universal Binary requires Apple's lipo tool"
        );
    }
    for format in &plan.formats {
        crate::platform::native_format(&plan.target, format)?;
    }
    for tool in crate::platform::required_tools(&plan.os, &plan.formats, &plan.config.signing) {
        ensure!(
            crate::platform::find(tool).is_some(),
            "required packaging tool is missing: {tool}"
        );
    }
    crate::resources::collect(&plan.config.bundle.resources)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema_version: u32,
    pub target: String,
    pub format: String,
    pub identifier: String,
    pub product_name: String,
    pub version: String,
    pub bin: String,
    pub executable: PathBuf,
    #[serde(default)]
    pub sidecars: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub icons: Vec<PathBuf>,
    pub deployment_target: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub customization: crate::config::Packager,
    pub trial: bool,
    pub execute_signing: bool,
    pub signing: crate::config::Signing,
    pub resources: Vec<crate::resources::Input>,
    pub file_associations: Vec<crate::config::FileAssociation>,
    pub protocols: Vec<crate::config::Protocol>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Response {
    pub schema_version: u32,
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    pub format: String,
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub verification: crate::signing::Verification,
}

pub(crate) fn command(request: &Path, response: &Path, directory: &Path) -> Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("__worker")
        .arg(request)
        .arg(response)
        .current_dir(directory)
        .env_clear();
    // A positive allowlist excludes all signing, notarization and updater credentials,
    // including future backend fallback variables. PATH remains required for OS tools.
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "WindowsSdkDir",
        "WindowsSdkVerBinPath",
        "WINDIR",
        "TEMP",
        "TMP",
        "TMPDIR",
        "XDG_CACHE_HOME",
        "LANG",
        "LC_ALL",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.env("CI", "true");
    Ok(command)
}

pub(crate) fn execute(request: &Request, directory: &Path) -> Result<Response> {
    let request_path = directory.join("worker-request.json");
    let response_path = directory.join("worker-response.json");
    report::atomic_new(&request_path, request)?;
    let status = process::status(&mut command(&request_path, &response_path, directory)?);
    if response_path.exists() {
        let response: Response = report::read(&response_path)?;
        if let Some(error) = &response.error {
            anyhow::bail!("packaging worker: {error}");
        }
        status.context("packaging worker failed")?;
        Ok(response)
    } else {
        status.context("packaging worker failed before returning a report")?;
        anyhow::bail!("packaging worker returned no report")
    }
}

pub(crate) fn worker(request: &Path, response: &Path) -> Result<()> {
    let request: Request = report::read(request)?;
    crate::platform::native_format(&request.target, &request.format)?;
    crate::signing::validate(
        &request.signing,
        crate::plan::target_os(&request.target)?,
        request.execute_signing,
    )?;
    ensure!(
        request.executable.is_absolute() && request.output_directory.is_absolute(),
        "worker paths must be absolute"
    );
    let result = package_and_verify(&request);
    let response_value = match &result {
        Ok(outputs) => Response {
            schema_version: 1,
            outputs: outputs.clone(),
            error: None,
        },
        Err(error) => Response {
            schema_version: 1,
            outputs: vec![],
            error: Some(format!("{error:#}")),
        },
    };
    report::atomic_new(response, &response_value)?;
    result.map(|_| ())
}

fn package_and_verify(request: &Request) -> Result<Vec<Output>> {
    crate::signing::sidecars(
        &request.signing,
        crate::plan::target_os(&request.target)?,
        request.execute_signing,
        &request.sidecars,
        &mut crate::signing::Native,
    )
    .context("sidecar signing")?;
    let mut outputs = cargo_packager::package(request).context("backend package generation")?;
    for output in &mut outputs {
        for path in &output.paths {
            ensure!(
                path.canonicalize()?
                    .starts_with(request.output_directory.canonicalize()?),
                "backend output escaped worker directory"
            );
            output.verification = crate::signing::finish(
                &request.signing,
                crate::plan::target_os(&request.target)?,
                path,
                &output.format,
                &mut crate::signing::Native,
            )
            .context("signing verification")?;
        }
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_environment_is_allowlisted() -> Result<()> {
        let cmd = command(Path::new("request"), Path::new("response"), Path::new("."))?;
        let keys: Vec<_> = cmd
            .get_envs()
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        assert!(
            !keys.iter().any(|key| key.contains("SIGN")
                || key.contains("TOKEN")
                || key.starts_with("APPLE_"))
        );
        assert!(keys.contains(&"CI".to_owned()));
        Ok(())
    }
    #[test]
    fn native_packaging_accepts_supported_formats_and_rejects_other_platforms() -> Result<()> {
        let (_temporary, mut plan, _) = crate::build::fixture()?;
        let (target, format) = match std::env::consts::OS {
            "macos" => (format!("{}-apple-darwin", std::env::consts::ARCH), "app"),
            "windows" => (
                format!("{}-pc-windows-msvc", std::env::consts::ARCH),
                "nsis",
            ),
            _ => (
                format!("{}-unknown-linux-gnu", std::env::consts::ARCH),
                "deb",
            ),
        };
        plan.target = target;
        plan.os = std::env::consts::OS.into();
        plan.formats = vec![format.into()];
        validate(&plan, false)?;
        plan.formats = vec!["unsupported".into()];
        assert!(validate(&plan, false).is_err());
        plan.formats.clear();
        assert!(validate(&plan, false).is_err());
        assert!(crate::platform::native_format("wasm32-unknown-unknown", "app").is_err());
        Ok(())
    }
}
