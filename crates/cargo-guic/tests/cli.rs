use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn fixture() -> Result<tempfile::TempDir> {
    let directory = tempfile::Builder::new()
        .prefix("cargo guic \u{65e5}\u{672c}\u{8a9e} ")
        .tempdir()?;
    fs::create_dir_all(directory.path().join("app/src"))?;
    fs::create_dir_all(directory.path().join("other/src"))?;
    fs::create_dir_all(directory.path().join(".cargo"))?;
    fs::write(
        directory.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["app", "other"]
resolver = "2"
[workspace.package]
version = "1.2.3"
authors = ["Fixture Author"]
license = "MIT"
[profile.ship]
inherits = "release"
opt-level = 1
"#,
    )?;
    for package in ["app", "other"] {
        fs::write(
            directory.path().join(package).join("Cargo.toml"),
            format!(
                r#"
[package]
name = "{package}"
version.workspace = true
authors.workspace = true
license.workspace = true
edition = "2021"
[features]
default = ["one"]
one = []
two = []
"#
            ),
        )?;
        fs::write(
            directory.path().join(package).join("src/main.rs"),
            "fn main() { println!(\"fixture application\"); }\n",
        )?;
    }
    fs::write(
        directory.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir = 'output with spaces'\n",
    )?;
    fs::write(
        directory.path().join("Cargo.lock"),
        "version = 3\n[[package]]\nname = 'app'\nversion = '1.2.3'\n[[package]]\nname = 'other'\nversion = '1.2.3'\n",
    )?;
    fs::write(
        directory.path().join("app/Guic.toml"),
        "schema-version = 1\n[app]\nidentifier = 'org.example.fixture'\n[build]\npackage = 'app'\n",
    )?;
    Ok(directory)
}

fn invoke(directory: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_cargo-guic"))
        .current_dir(directory)
        .arg("guic")
        .arg("--json")
        .args(args)
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("MACOSX_DEPLOYMENT_TARGET")
        .output()?)
}
fn success(output: Output) -> Result<Value> {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn failure(output: Output, expected: &str) -> Result<()> {
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert!(value.to_string().contains(expected), "{value}");
    Ok(())
}

#[test]
fn workspace_selection_inheritance_and_config_discovery() -> Result<()> {
    let dir = fixture()?;
    failure(invoke(dir.path(), &["inspect"])?, "select one application")?;
    let inspection = success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    assert_eq!(inspection["plan"]["project"]["version"], "1.2.3");
    assert_eq!(
        inspection["plan"]["project"]["authors"][0],
        "Fixture Author"
    );
    assert!(
        inspection["plan"]["project"]["target_directory"]
            .as_str()
            .context("target dir")?
            .ends_with("output with spaces")
    );
    let explicit = success(invoke(
        dir.path(),
        &["inspect", "--config", "app/Guic.toml"],
    )?)?;
    assert_eq!(explicit["plan"]["project"]["package"], "app");
    failure(
        invoke(
            dir.path(),
            &["inspect", "--config", "app/Guic.toml", "-p", "other"],
        )?,
        "conflicts",
    )?;
    Ok(())
}

#[test]
fn precedence_paths_unknown_fields_and_no_hook_execution() -> Result<()> {
    let dir = fixture()?;
    fs::write(dir.path().join("app/icon.png"), b"icon fixture")?;
    fs::write(
        dir.path().join("app/Guic.toml"),
        format!(
            r#"
schema-version = 1
[app]
identifier = 'org.example.fixture'
[build]
features = ['one']
[bundle]
icons = ['icon.png']
[platform.{}.build]
features = ['two']
[hooks.before-build]
program = 'does-not-exist-guic-fixture'
args = ['secret-hook-argument']
cwd = '.'
"#,
            std::env::consts::OS
        ),
    )?;
    let inspection = success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    assert_eq!(inspection["plan"]["features"], serde_json::json!(["two"]));
    assert!(
        Path::new(
            inspection["bundle"]["icons"][0]
                .as_str()
                .context("icon path")?
        )
        .ends_with(Path::new("app").join("icon.png"))
    );
    assert!(!inspection.to_string().contains("secret-hook-argument"));
    let overridden = success(invoke(
        dir.path(),
        &[
            "inspect",
            "-p",
            "app",
            "--features",
            "one",
            "--profile",
            "ship",
        ],
    )?)?;
    assert_eq!(overridden["plan"]["features"], serde_json::json!(["one"]));
    assert_eq!(overridden["plan"]["provenance"]["build.features"], "cli");
    let mut text = fs::read_to_string(dir.path().join("app/Guic.toml"))?;
    text.push_str("\n[backend.cargo-packager]\nsigning-password = 'do-not-leak-this'\n");
    fs::write(dir.path().join("app/Guic.toml"), text)?;
    let rejected = invoke(dir.path(), &["inspect", "-p", "app"])?;
    assert!(!String::from_utf8_lossy(&rejected.stdout).contains("do-not-leak-this"));
    failure(rejected, "unknown keys")?;
    Ok(())
}

#[test]
fn real_build_custom_profile_snapshot_and_record_constraints() -> Result<()> {
    let dir = fixture()?;
    let record = success(invoke(
        dir.path(),
        &["build", "-p", "app", "--profile", "ship", "--no-bundle"],
    )?)?;
    assert_eq!(record["profile"], "ship");
    assert_eq!(record["application_version"], "1.2.3");
    assert_eq!(record["locked"], true);
    let snapshot = Path::new(record["snapshot_path"].as_str().context("snapshot")?);
    let original = Path::new(record["executable_path"].as_str().context("original")?);
    assert_ne!(snapshot, original);
    let bytes = fs::read(snapshot)?;
    fs::write(original, b"later Cargo output")?;
    assert_eq!(fs::read(snapshot)?, bytes);
    failure(
        invoke(dir.path(), &["bundle", "-p", "app", "--profile", "release"])?,
        "0 compatible records",
    )?;
    failure(
        invoke(dir.path(), &["bundle", "-p", "app", "--profile", "ship"])?,
        "no package formats",
    )?;
    fs::write(snapshot, b"modified")?;
    failure(
        invoke(dir.path(), &["bundle", "-p", "app", "--profile", "ship"])?,
        "modified",
    )?;
    fs::remove_file(snapshot)?;
    failure(
        invoke(dir.path(), &["bundle", "-p", "app", "--profile", "ship"])?,
        "missing",
    )?;
    Ok(())
}

#[test]
fn init_does_not_overwrite_and_requires_explicit_identity() -> Result<()> {
    let dir = fixture()?;
    success(invoke(
        dir.path(),
        &["init", "-p", "other", "--identifier", "org.example.other"],
    )?)?;
    let path = dir.path().join("other/Guic.toml");
    let first = fs::read(&path)?;
    failure(
        invoke(
            dir.path(),
            &["init", "-p", "other", "--identifier", "org.example.changed"],
        )?,
        "never overwritten",
    )?;
    assert_eq!(fs::read(path)?, first);
    let inspection = success(invoke(dir.path(), &["inspect", "-p", "other"])?)?;
    let expected = match std::env::consts::OS {
        "macos" => serde_json::json!(["app", "dmg"]),
        "windows" => serde_json::json!(["nsis"]),
        _ => serde_json::json!(["deb", "appimage"]),
    };
    assert_eq!(inspection["plan"]["formats"], expected);
    Ok(())
}

#[test]
fn configured_packaging_reaches_build_hooks() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    let mut config = fs::read_to_string(&path)?;
    let (os, format) = match std::env::consts::OS {
        "macos" => ("macos", "app"),
        "windows" => ("windows", "nsis"),
        _ => ("linux", "deb"),
    };
    config.push_str(&format!("\n[bundle.formats]\n{os} = ['{format}']\n[hooks.before-build]\nprogram = 'missing-fixture-command'\nargs = []\ncwd = '.'\n"));
    fs::write(path, config)?;
    failure(invoke(dir.path(), &["build", "-p", "app"])?, "hook failed")?;
    failure(
        invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?,
        "hook failed",
    )?;
    Ok(())
}

#[test]
fn multi_binary_selection_and_conflicts() -> Result<()> {
    let dir = fixture()?;
    fs::create_dir_all(dir.path().join("app/src/bin"))?;
    fs::write(dir.path().join("app/src/bin/extra.rs"), "fn main() {}")?;
    failure(
        invoke(dir.path(), &["inspect", "-p", "app"])?,
        "unambiguous --bin",
    )?;
    success(invoke(
        dir.path(),
        &["inspect", "-p", "app", "--bin", "extra"],
    )?)?;
    let path = dir.path().join("app/Guic.toml");
    let text = fs::read_to_string(&path)? + "bin = 'app'\n";
    fs::write(path, text)?;
    failure(
        invoke(dir.path(), &["inspect", "-p", "app", "--bin", "extra"])?,
        "conflicts",
    )?;
    Ok(())
}

#[test]
fn cargo_target_configuration_and_feature_constraints() -> Result<()> {
    let dir = fixture()?;
    let initial = success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    let target = initial["plan"]["host"].as_str().context("host")?;
    fs::write(
        dir.path().join(".cargo/config.toml"),
        format!("[build]\ntarget = '{target}'\ntarget-dir = 'custom target'\n"),
    )?;
    let resolved = success(invoke(
        dir.path(),
        &[
            "inspect",
            "-p",
            "app",
            "--no-default-features",
            "--features",
            "two,one",
        ],
    )?)?;
    assert_eq!(resolved["plan"]["target"], target);
    assert_eq!(resolved["plan"]["default_features"], false);
    assert_eq!(
        resolved["plan"]["features"],
        serde_json::json!(["one", "two"])
    );
    failure(
        invoke(
            dir.path(),
            &["inspect", "-p", "app", "--target", "wasm32-unknown-unknown"],
        )?,
        "unsupported desktop target",
    )?;
    Ok(())
}

#[test]
fn dev_forwards_arguments_without_polluting_json() -> Result<()> {
    let dir = fixture()?;
    fs::write(
        dir.path().join("app/src/main.rs"),
        "fn main() { println!(\"APP:{}\", std::env::args().nth(1).unwrap()); }\n",
    )?;
    let output = invoke(
        dir.path(),
        &["dev", "-p", "app", "--", "argument with spaces"],
    )?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("APP:argument with spaces"));
    let record = success(output)?;
    assert_eq!(record["profile"], "dev");
    Ok(())
}

#[test]
fn dev_reports_application_failure_without_success_report() -> Result<()> {
    let dir = fixture()?;
    fs::write(
        dir.path().join("app/src/main.rs"),
        "fn main() { std::process::exit(17); }\n",
    )?;
    failure(
        invoke(dir.path(), &["dev", "-p", "app"])?,
        "child process exited",
    )?;
    Ok(())
}

#[test]
fn record_ambiguity_and_schema_migration_are_explicit() -> Result<()> {
    let dir = fixture()?;
    let first = success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    failure(
        invoke(dir.path(), &["bundle", "-p", "app"])?,
        "2 compatible records",
    )?;
    let snapshot = Path::new(first["snapshot_path"].as_str().context("snapshot")?);
    let root = snapshot
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .context("artifact root")?;
    let record_path = root
        .join("records")
        .join(format!("{}.json", first["id"].as_str().context("id")?));
    let mut invalid = first;
    invalid["schema_version"] = 100.into();
    fs::write(&record_path, serde_json::to_vec(&invalid)?)?;
    failure(
        invoke(
            dir.path(),
            &[
                "bundle",
                "-p",
                "app",
                "--build-record",
                record_path.to_str().context("record path")?,
            ],
        )?,
        "unsupported record",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn generated_build_inputs_are_hashed_after_hooks_and_changes_invalidate_records() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    let text = fs::read_to_string(&path)?
        + "inputs = ['generated.txt']\n[hooks.before-build]\nprogram = 'sh'\nargs = ['-c', 'printf generated > generated.txt']\ncwd = '.'\n";
    fs::write(&path, text)?;
    let record = success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    assert_eq!(
        record["build_input_hashes"]
            .as_object()
            .context("hashes")?
            .len(),
        1
    );
    failure(
        invoke(dir.path(), &["bundle", "-p", "app"])?,
        "no package formats",
    )?;
    fs::write(dir.path().join("app/generated.txt"), b"changed build input")?;
    failure(
        invoke(dir.path(), &["bundle", "-p", "app"])?,
        "0 compatible records",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn nonzero_hooks_never_create_records() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    let text = fs::read_to_string(&path)?
        + "\n[hooks.before-build]\nprogram = 'sh'\nargs = ['-c', 'exit 17']\ncwd = '.'\n";
    fs::write(path, text)?;
    failure(
        invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?,
        "exit status: 17",
    )?;
    let reports: Vec<_> = walkdir::WalkDir::new(dir.path())
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
        .collect();
    assert!(reports.is_empty());
    Ok(())
}

#[cfg(unix)]
#[test]
fn cancellation_terminates_owned_hook_processes_without_success() -> Result<()> {
    assert_cancellation_terminates_child(false)
}

#[cfg(unix)]
#[test]
fn cancellation_terminates_owned_dev_application_without_success() -> Result<()> {
    assert_cancellation_terminates_child(true)
}

#[cfg(unix)]
fn assert_cancellation_terminates_child(dev: bool) -> Result<()> {
    use std::time::{Duration, Instant};
    let dir = fixture()?;
    if dev {
        fs::write(
            dir.path().join("app/src/main.rs"),
            r#"fn main() {
                std::fs::write("child.pid", std::process::id().to_string()).unwrap();
                std::thread::sleep(std::time::Duration::from_secs(60));
            }"#,
        )?;
    } else {
        let path = dir.path().join("app/Guic.toml");
        let text = fs::read_to_string(&path)?
            + "\n[hooks.before-build]\nprogram = 'sh'\nargs = ['-c', 'echo $$ > child.pid; exec sleep 60']\ncwd = '.'\n";
        fs::write(path, text)?;
    }
    let args = if dev {
        vec!["--json", "dev", "-p", "app"]
    } else {
        vec!["--json", "build", "-p", "app", "--no-bundle"]
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_cargo-guic"))
        .current_dir(dir.path())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    let pid_file = dir.path().join("app/child.pid");
    while !pid_file.is_file() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if !pid_file.is_file() {
        child.kill()?;
        child.wait()?;
        anyhow::bail!("child did not start before timeout");
    }
    let owned_pid: i32 = fs::read_to_string(pid_file)?.trim().parse()?;
    // Send an interrupt to the CLI, which owns the separate child process group.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    while child.try_wait()?.is_none() && started.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait()?.is_none() {
        child.kill()?;
        unsafe {
            libc::kill(owned_pid, libc::SIGKILL);
        }
        child.wait()?;
        anyhow::bail!("cancellation did not stop the CLI");
    }
    failure(child.wait_with_output()?, "cancelled")?;
    assert_eq!(
        unsafe { libc::kill(owned_pid, 0) },
        -1,
        "owned process survived cancellation"
    );
    Ok(())
}

#[test]
fn configuration_schema_is_current() -> Result<()> {
    let directory = fixture()?;
    let schema = success(invoke(directory.path(), &["schema"])?)?;
    let committed: Value =
        serde_json::from_str(include_str!("../../../docs/cargo-guic.schema.json"))?;
    assert_eq!(
        schema, committed,
        "regenerate docs/cargo-guic.schema.json with cargo guic schema"
    );
    Ok(())
}

#[test]
fn resource_contract_signing_and_doctor_are_explicit() -> Result<()> {
    let directory = fixture()?;
    let path = directory.path().join("app/Guic.toml");
    let text = fs::read_to_string(&path)?;
    fs::write(
        &path,
        text.clone() + "\n[[bundle.resources]]\nsource = 'file.txt'\ndestination = 'file.txt'\n",
    )?;
    failure(
        invoke(directory.path(), &["inspect", "-p", "app"])?,
        "resource-contract",
    )?;
    fs::write(
        &path,
        text.clone() + "\n[signing]\ncode-signing = 'required'\n",
    )?;
    failure(
        invoke(directory.path(), &["build", "-p", "app"])?,
        "--execute-signing",
    )?;
    let diagnosis = invoke(directory.path(), &["doctor", "-p", "app"])?;
    assert!(!diagnosis.status.success());
    let report: Value = serde_json::from_slice(&diagnosis.stdout)?;
    assert_eq!(report["ready"], false);
    assert!(
        report["checks"]
            .as_array()
            .context("checks")?
            .iter()
            .any(|check| check["name"] == "packaging-prerequisites" && check["passed"] == false)
    );
    assert!(
        !report
            .to_string()
            .contains("needs explicit --execute-signing")
    );
    fs::write(
        &path,
        text.replace("schema-version = 1", "schema-version = 2"),
    )?;
    failure(
        invoke(directory.path(), &["inspect", "-p", "app"])?,
        "migrate",
    )?;
    Ok(())
}

#[test]
fn compile_and_record_use_the_same_feature_policy() -> Result<()> {
    let dir = fixture()?;
    fs::write(
        dir.path().join("app/src/main.rs"),
        r#"
#[cfg(not(feature = "two"))]
compile_error!("feature two must be enabled");
#[cfg(feature = "one")]
compile_error!("default feature must be disabled");
fn main() {}
"#,
    )?;
    let record = success(invoke(
        dir.path(),
        &[
            "build",
            "-p",
            "app",
            "--no-bundle",
            "--features",
            "two",
            "--no-default-features",
        ],
    )?)?;
    assert_eq!(record["features"], serde_json::json!(["two"]));
    assert_eq!(record["default_features"], false);
    failure(
        invoke(dir.path(), &["bundle", "-p", "app"])?,
        "0 compatible records",
    )?;
    Ok(())
}

#[test]
fn external_resources_use_explicit_contract_and_dev_root() -> Result<()> {
    let dir = fixture()?;
    fs::create_dir_all(dir.path().join("app/data/nested"))?;
    fs::write(
        dir.path().join("app/data/nested/value.txt"),
        "resource fixture",
    )?;
    let path = dir.path().join("app/Guic.toml");
    let text = fs::read_to_string(&path)?
        + "\n[bundle]\nresource-contract = 'application-managed-v1'\n[[bundle.resources]]\nsource = 'data/**/*.txt'\ndestination = 'data'\n";
    fs::write(&path, text)?;
    fs::write(
        dir.path().join("app/src/main.rs"),
        r#"
fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("GUIC_RESOURCE_DIR").expect("resource root"));
    assert!(root.is_absolute());
    assert_eq!(std::env::current_dir().unwrap().canonicalize().unwrap(), std::path::Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize().unwrap());
    println!("RESOURCE:{}", std::fs::read_to_string(root.join("data/nested/value.txt")).expect("resource"));
}
"#,
    )?;
    let inspection = success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    assert_eq!(
        inspection["plan"]["resource_layout"],
        "application-managed-v1"
    );
    let output = invoke(dir.path(), &["dev", "-p", "app"])?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("RESOURCE:resource fixture"));
    let record = success(output)?;
    assert_eq!(record["resource_layout"], "application-managed-v1");
    // Absolute selectors must work from an unrelated directory, including when
    // the project and staged resource paths contain spaces and Japanese text.
    let outside = tempfile::tempdir()?;
    let output = invoke(
        outside.path(),
        &["dev", "--config", path.to_str().context("config path")?],
    )?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("RESOURCE:resource fixture"));
    success(output)?;
    Ok(())
}

#[test]
fn trial_requires_explicit_record_and_signing_authorization() -> Result<()> {
    let dir = fixture()?;
    failure(
        invoke(dir.path(), &["package-trial", "-p", "app"])?,
        "--build-record",
    )?;
    let path = dir.path().join("app/Guic.toml");
    let format = match std::env::consts::OS {
        "macos" => "app",
        "windows" => "nsis",
        _ => "deb",
    };
    fs::write(
        &path,
        fs::read_to_string(&path)? + "\n[signing]\ncode-signing = 'required'\n",
    )?;
    failure(
        invoke(
            dir.path(),
            &[
                "package-trial",
                "-p",
                "app",
                "--formats",
                format,
                "--build-record",
                "missing.json",
            ],
        )?,
        "--execute-signing",
    )?;
    Ok(())
}

// This test generates a real package candidate. It is deliberately never run by
// ordinary tests or CI; only the maintainer may execute the acceptance operation.
#[test]
#[ignore = "Manual native packaging acceptance; creates a candidate artifact"]
fn manual_native_backend_candidate() -> Result<()> {
    let dir = fixture()?;
    let record = success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    let snapshot = Path::new(record["snapshot_path"].as_str().context("snapshot")?);
    let record_path = snapshot
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .context("artifact root")?
        .join("records")
        .join(format!("{}.json", record["id"].as_str().context("id")?));
    let format = match std::env::consts::OS {
        "macos" => "app",
        "windows" => "nsis",
        _ => "deb",
    };
    let candidate = success(invoke(
        dir.path(),
        &[
            "package-trial",
            "-p",
            "app",
            "--formats",
            format,
            "--build-record",
            record_path.to_str().context("record path")?,
        ],
    )?)?;
    assert_eq!(candidate["trial"], true);
    assert_eq!(candidate["acceptance"], "not-verified");
    assert_eq!(candidate["complete"], true);
    assert!(
        Path::new(
            candidate["outputs"][0]["path"]
                .as_str()
                .context("output path")?
        )
        .exists()
    );
    let packaged = success(invoke(
        dir.path(),
        &[
            "bundle",
            "-p",
            "app",
            "--formats",
            format,
            "--build-record",
            record_path.to_str().context("record path")?,
        ],
    )?)?;
    assert_eq!(packaged["trial"], false);
    assert_eq!(packaged["complete"], true);
    let built = success(invoke(
        dir.path(),
        &["build", "-p", "app", "--formats", format],
    )?)?;
    assert_eq!(built["trial"], false);
    assert_eq!(built["complete"], true);
    for report in [&packaged, &built] {
        assert!(Path::new(report["outputs"][0]["path"].as_str().context("artifact")?).exists());
    }
    eprintln!("Packaging evidence preserved in {}", dir.keep().display());
    eprintln!("{}", serde_json::to_string_pretty(&candidate)?);
    Ok(())
}

#[test]
fn plist_cannot_override_cli_owned_identity() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    fs::write(
        &path,
        fs::read_to_string(&path)?
            + "\n[backend.cargo-packager]\nmacos-info-plist = 'extra.plist'\n",
    )?;
    fs::write(
        dir.path().join("app/extra.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>unexpected.id</string></dict></plist>"#,
    )?;
    failure(
        invoke(dir.path(), &["inspect", "-p", "app"])?,
        "owned by cargo-guic",
    )?;
    fs::write(
        dir.path().join("app/extra.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>NSMicrophoneUsageDescription</key><string>Example usage description</string></dict></plist>"#,
    )?;
    success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    Ok(())
}

#[test]
fn native_resource_contract_and_runtime_policy_are_inspectable() -> Result<()> {
    let dir = fixture()?;
    let config = dir.path().join("app/Guic.toml");
    fs::write(dir.path().join("app/data.txt"), "fixture")?;
    let original = fs::read_to_string(&config)?;
    fs::write(
        &config,
        original.clone()
            + "\n[bundle]\nresource-contract = 'guic-native-v1'\n[[bundle.resources]]\nsource = 'data.txt'\ndestination = 'data.txt'\n[runtime]\nwebview = true\nglibc-max = '2.31'\ndynamic-libraries = ['example.dll']\n",
    )?;
    let inspected = success(invoke(dir.path(), &["inspect", "-p", "app"])?)?;
    assert_eq!(inspected["resource_layout"], "guic-native-v1");
    assert_eq!(inspected["runtime"]["webview"], true);
    fs::write(
        &config,
        original + "\n[runtime]\ndynamic-libraries = ['--bad-option']\n",
    )?;
    failure(
        invoke(dir.path(), &["inspect", "-p", "app"])?,
        "plain package or library names",
    )?;
    Ok(())
}

#[test]
fn doctor_inspects_verified_snapshot_without_running_it() -> Result<()> {
    let dir = fixture()?;
    #[cfg(target_os = "linux")]
    {
        let config = dir.path().join("app/Guic.toml");
        fs::write(
            &config,
            fs::read_to_string(&config)? + "\n[runtime]\nglibc-max = '2.0'\n",
        )?;
    }
    fs::write(
        dir.path().join("app/src/main.rs"),
        "fn main() { panic!(\"must not execute during diagnostics\"); }",
    )?;
    let record = success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    let path = dir.path().join("record.json");
    fs::write(&path, serde_json::to_vec(&record)?)?;
    let report = invoke(
        dir.path(),
        &[
            "doctor",
            "-p",
            "app",
            "--build-record",
            path.to_str().context("path")?,
        ],
    )?;
    let value: Value = serde_json::from_slice(&report.stdout)?;
    assert!(
        value["checks"]
            .as_array()
            .context("checks")?
            .iter()
            .any(|c| c["name"] == "binary-import-inspection")
    );
    #[cfg(target_os = "linux")]
    {
        let checks = value["checks"].as_array().context("checks")?;
        assert!(
            checks
                .iter()
                .any(|c| { c["name"] == "binary-import-inspection" && c["passed"] == true })
        );
        assert!(
            checks
                .iter()
                .any(|c| { c["name"] == "runtime-library:libc.so.6" && c["passed"] == true })
        );
        assert!(checks.iter().any(|c| {
            c["name"] == "glibc-baseline" && c["required"] == true && c["passed"] == false
        }));
        assert!(!report.status.success());
        assert_eq!(value["ready"], false);
    }
    fs::write(
        record["snapshot_path"].as_str().context("snapshot")?,
        "modified",
    )?;
    failure(
        invoke(
            dir.path(),
            &[
                "doctor",
                "-p",
                "app",
                "--build-record",
                path.to_str().context("path")?,
            ],
        )?,
        "modified",
    )?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_resources_require_handshake_and_embed_version() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    fs::write(
        &path,
        fs::read_to_string(&path)?
            + "\n[build.windows-resources]\ncompany-name = 'Fixture Company'\n",
    )?;
    failure(
        invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?,
        "not acknowledged",
    )?;
    let helper = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../guic-build")
        .canonicalize()?;
    let dependency = toml::Value::String(helper.to_string_lossy().into_owned()).to_string();
    let manifest = dir.path().join("app/Cargo.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)?
            + &format!("\n[build-dependencies]\nguic-build = {{ path = {dependency} }}\n"),
    )?;
    fs::write(
        dir.path().join("app/build.rs"),
        "fn main() { guic_build::build().expect(\"native resources\"); }",
    )?;
    let record = success(invoke(
        dir.path(),
        &["build", "-p", "app", "--no-bundle", "--unlocked"],
    )?)?;
    let script = dir.path().join("inspect.ps1");
    fs::write(
        &script,
        "param([string]$Binary)\n(Get-Item -LiteralPath $Binary).VersionInfo | Select-Object FileVersion,CompanyName | ConvertTo-Json\n",
    )?;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(script)
        .arg(record["snapshot_path"].as_str().context("snapshot")?)
        .output()?;
    assert!(output.status.success());
    let version: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(version["FileVersion"], "1.2.3");
    assert_eq!(version["CompanyName"], "Fixture Company");
    Ok(())
}

#[cfg(unix)]
#[test]
fn build_input_names_cannot_overwrite_the_executable_snapshot() -> Result<()> {
    let dir = fixture()?;
    let manifest = dir.path().join("app/Cargo.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)? + "\n[[bin]]\nname = 'input-0'\npath = 'src/main.rs'\n",
    )?;
    let config = dir.path().join("app/Guic.toml");
    fs::write(
        &config,
        fs::read_to_string(&config)? + "bin = 'input-0'\ninputs = ['data.txt']\n",
    )?;
    fs::write(
        dir.path().join("app/data.txt"),
        b"build input, not an executable",
    )?;
    let record = success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    let snapshot = record["snapshot_path"].as_str().context("snapshot")?;
    let executable = record["executable_path"].as_str().context("executable")?;
    assert!(
        fs::read(snapshot)? == fs::read(executable)?,
        "snapshot must preserve executable bytes"
    );
    success(invoke(dir.path(), &["dev", "-p", "app"])?)?;
    Ok(())
}

#[test]
fn normal_bundle_reaches_packaging_hooks_without_rebuilding() -> Result<()> {
    let dir = fixture()?;
    success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    let path = dir.path().join("app/Guic.toml");
    let (os, format) = match std::env::consts::OS {
        "macos" => ("macos", "app"),
        "windows" => ("windows", "nsis"),
        _ => ("linux", "deb"),
    };
    fs::write(
        &path,
        fs::read_to_string(&path)?
            + &format!(
                "\n[bundle.formats]\n{os} = ['{format}']\n[hooks.before-bundle]\nprogram = 'missing-bundle-fixture-command'\ncwd = '.'\n"
            ),
    )?;
    // Packaging must use the recorded executable even when current sources cannot compile.
    fs::write(
        dir.path().join("app/src/main.rs"),
        "compile_error!(\"must not rebuild\");",
    )?;
    failure(invoke(dir.path(), &["bundle", "-p", "app"])?, "hook failed")?;
    fs::write(dir.path().join("app/src/main.rs"), "fn main() {}")?;
    // The combined build path must also reach packaging after recording the binary.
    failure(invoke(dir.path(), &["build", "-p", "app"])?, "hook failed")?;
    Ok(())
}

#[test]
fn normal_commands_require_explicit_signing_execution() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    fs::write(
        &path,
        fs::read_to_string(&path)? + "\n[signing]\ncode-signing = 'required'\n",
    )?;
    success(invoke(dir.path(), &["build", "-p", "app", "--no-bundle"])?)?;
    for command in ["build", "bundle"] {
        failure(
            invoke(dir.path(), &[command, "-p", "app"])?,
            "--execute-signing",
        )?;
        let output = invoke(dir.path(), &[command, "-p", "app", "--execute-signing"])?;
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stdout)?;
        let expected = match std::env::consts::OS {
            "macos" => "signing.macos-identity",
            "windows" => "signing.windows-thumbprint",
            _ => "not defined for Linux",
        };
        assert!(error.to_string().contains(expected), "{error}");
    }
    failure(
        invoke(
            dir.path(),
            &["build", "-p", "app", "--no-bundle", "--execute-signing"],
        )?,
        "cannot be used with",
    )?;
    Ok(())
}

#[test]
fn doctor_checks_native_packaging_without_running_hooks() -> Result<()> {
    let dir = fixture()?;
    let path = dir.path().join("app/Guic.toml");
    let (os, format) = match std::env::consts::OS {
        "macos" => ("macos", "app"),
        "windows" => ("windows", "nsis"),
        _ => ("linux", "deb"),
    };
    fs::write(
        &path,
        fs::read_to_string(&path)?
            + &format!(
                "\n[bundle.formats]\n{os} = ['{format}']\n[hooks.before-bundle]\nprogram = 'missing-bundle-fixture-command'\ncwd = '.'\n"
            ),
    )?;
    let output = invoke(dir.path(), &["doctor", "-p", "app"])?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert!(
        report["checks"]
            .as_array()
            .context("checks")?
            .iter()
            .any(|check| check["name"] == "packaging-prerequisites" && check["passed"] == true)
    );
    assert!(!dir.path().join("output with spaces/guic").exists());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "Requires both macOS Rust targets and Xcode command line tools"]
fn universal_binary_contains_both_architectures() -> Result<()> {
    let dir = fixture()?;
    let result = success(invoke(
        dir.path(),
        &[
            "build",
            "-p",
            "app",
            "--no-bundle",
            "--target",
            "universal-apple-darwin",
        ],
    )?)?;
    assert_eq!(result["target"], "universal-apple-darwin");
    let snapshot = result["snapshot_path"].as_str().context("snapshot")?;
    let verification = Command::new("lipo")
        .arg(snapshot)
        .args(["-verify_arch", "arm64", "x86_64"])
        .output()?;
    assert!(
        verification.status.success(),
        "{}",
        String::from_utf8_lossy(&verification.stderr)
    );
    // Record selection must preserve the combined target identity.
    failure(
        invoke(
            dir.path(),
            &["bundle", "-p", "app", "--target", "universal-apple-darwin"],
        )?,
        "no package formats",
    )?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn watch_restarts_and_recovers_after_a_build_failure() -> Result<()> {
    use std::time::{Duration, Instant};
    struct Running(std::process::Child);
    impl Drop for Running {
        fn drop(&mut self) {
            unsafe {
                libc::kill(self.0.id() as i32, libc::SIGINT);
            }
            for _ in 0..100 {
                if self.0.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn wait_for(mut condition: impl FnMut() -> bool) -> Result<()> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(30) {
            if condition() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        anyhow::bail!("watcher did not reach expected state");
    }
    fn source(version: &str) -> String {
        format!(
            r#"fn main() {{
            std::fs::write("run.log", format!("{version}:{{}}", std::process::id())).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
        }}"#
        )
    }
    let dir = fixture()?;
    let src = dir.path().join("app/src/main.rs");
    let marker = dir.path().join("app/run.log");
    let logs = tempfile::tempfile()?;
    fs::write(&src, source("one"))?;
    let mut running = Running(
        Command::new(env!("CARGO_BIN_EXE_cargo-guic"))
            .current_dir(dir.path())
            .args([
                "--json",
                "dev",
                "-p",
                "app",
                "--watch",
                "--watch-ignore",
                "app/run.log",
            ])
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("CARGO_BUILD_TARGET")
            .stdout(std::process::Stdio::null())
            .stderr(logs.try_clone()?)
            .spawn()?,
    );
    wait_for(|| fs::read_to_string(&marker).is_ok_and(|s| s.starts_with("one:")))?;
    let first: i32 = fs::read_to_string(&marker)?
        .split(':')
        .nth(1)
        .context("pid")?
        .parse()?;
    fs::write(&src, source("two"))?;
    wait_for(|| fs::read_to_string(&marker).is_ok_and(|s| s.starts_with("two:")))?;
    assert_eq!(
        unsafe { libc::kill(first, 0) },
        -1,
        "old app survived restart"
    );
    fs::write(&src, "compile_error!(\"watch failure fixture\");")?;
    wait_for(|| {
        use std::io::{Read, Seek};
        let mut file = logs.try_clone().expect("log clone");
        file.rewind().expect("rewind");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("logs");
        text.contains("watching for changes")
    })?;
    assert!(running.0.try_wait()?.is_none());
    fs::write(&src, source("three"))?;
    wait_for(|| fs::read_to_string(&marker).is_ok_and(|s| s.starts_with("three:")))?;
    let last: i32 = fs::read_to_string(&marker)?
        .split(':')
        .nth(1)
        .context("pid")?
        .parse()?;
    drop(running);
    assert_eq!(
        unsafe { libc::kill(last, 0) },
        -1,
        "app survived watcher shutdown"
    );
    Ok(())
}
