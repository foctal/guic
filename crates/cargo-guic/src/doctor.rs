use crate::{backend, plan::BuildPlan};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Diagnosis {
    pub schema_version: u32,
    pub application: String,
    pub target: String,
    pub ready: bool,
    pub checks: Vec<Check>,
    pub tool_paths: std::collections::BTreeMap<String, std::path::PathBuf>,
    pub tool_versions: std::collections::BTreeMap<String, String>,
}
#[derive(Serialize)]
pub(crate) struct Check {
    pub(crate) name: String,
    pub(crate) passed: Option<bool>,
    pub(crate) required: bool,
    pub(crate) detail: String,
}

pub(crate) fn diagnose(plan: &BuildPlan, executable: Option<&std::path::Path>) -> Diagnosis {
    let mut checks = vec![Check {
        name: "configuration".into(),
        passed: Some(true),
        required: true,
        detail: "Application, target, formats and configured paths resolved".into(),
    }];
    let build_tools: &[&str] = match plan.os.as_str() {
        "macos" => &["xcrun", "clang"],
        "windows" => &["link.exe", "rc.exe", "fxc.exe"],
        _ => &["cc", "pkg-config"],
    };
    let mut tools = build_tools.to_vec();
    tools.extend(crate::platform::required_tools(
        &plan.os,
        &plan.formats,
        &plan.config.signing,
    ));
    tools.sort();
    tools.dedup();
    let mut tool_paths = std::collections::BTreeMap::new();
    let mut tool_versions = plan.tool_versions.clone();
    for tool in tools {
        let path = crate::platform::find(tool);
        let found = path.is_some();
        if let Some(path) = path {
            tool_paths.insert(tool.to_owned(), path.clone());
            if ["clang", "cc", "pkg-config", "xcrun"].contains(&tool)
                && let Ok(bytes) =
                    crate::process::capture(std::process::Command::new(path).arg("--version"))
            {
                tool_versions.insert(
                    tool.to_owned(),
                    String::from_utf8_lossy(&bytes).trim().into(),
                );
            }
        }
        checks.push(Check {
            name: tool.into(),
            passed: Some(found),
            required: true,
            detail: if found {
                "Found in PATH or platform SDK"
            } else {
                "Missing from PATH and platform SDK; initialize the native developer environment or install build tools manually"
            }
            .into(),
        });
    }
    if !plan.formats.is_empty()
        || plan.config.signing.code_signing == crate::config::Policy::Required
        || plan.config.signing.notarization == crate::config::Policy::Required
    {
        let result = backend::validate(plan);
        checks.push(Check {
            name: "packaging-acceptance".into(),
            passed: Some(result.is_ok()),
            required: true,
            detail: result
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Validated".into()),
        });
    }
    if !plan.formats.is_empty() {
        let trial = backend::validate_trial(plan, false);
        checks.push(Check { name: "unsigned-candidate-prerequisites".into(), passed: Some(trial.is_ok()), required: false,
            detail: trial.err().map(|e| e.to_string()).unwrap_or_else(|| "package-trial can exercise the native backend; normal distribution is disabled in this version".into()) });
    }
    if let Ok(text) = std::fs::read_to_string(plan.project.workspace_root.join("Cargo.lock"))
        && let Ok(lock) = toml::from_str::<toml::Value>(&text)
        && let Some(packages) = lock.get("package").and_then(toml::Value::as_array)
    {
        for package in packages {
            if let (Some(name), Some(version)) = (
                package.get("name").and_then(toml::Value::as_str),
                package.get("version").and_then(toml::Value::as_str),
            ) && ["gpui", "guic-gpui", "guic", "guic-build", "guic-assets"].contains(&name)
            {
                tool_versions.insert(format!("lockfile:{name}@{version}"), version.into());
            }
        }
    }
    checks.extend(crate::runtime::diagnose(plan, executable));
    checks.push(Check { name: "runtime-acceptance".into(), passed: None,
        required: false,
        detail: "Not verified: installed launch, GUI, WebView, linked libraries and glibc baseline require native acceptance; no runtime capability is inferred from dependency names".into() });
    Diagnosis {
        schema_version: 1,
        application: plan.identifier.clone(),
        target: plan.target.clone(),
        ready: checks.iter().all(|c| !c.required || c.passed == Some(true)),
        checks,
        tool_paths,
        tool_versions,
    }
}
