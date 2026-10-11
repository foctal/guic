use crate::{
    config::{Policy, Signing},
    process,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::Path, process::Command};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Outcome {
    #[default]
    Disabled,
    Verified,
    Failed,
    NotVerified,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Verification {
    pub code_signing: Outcome,
    pub notarization: Outcome,
    pub stapling: Outcome,
}

pub(crate) fn validate(config: &Signing, os: &str, execute: bool) -> Result<()> {
    if config.code_signing == Policy::Off {
        ensure!(
            config.notarization == Policy::Off,
            "notarization requires code signing"
        );
        return Ok(());
    }
    ensure!(
        execute,
        "required signing needs explicit --execute-signing on package-trial"
    );
    match os {
        "macos" => {
            let identity = config
                .macos_identity
                .as_deref()
                .context("required code signing needs signing.macos-identity")?;
            ensure!(
                !identity.trim().is_empty() && identity != "-",
                "required signing needs a non-ad-hoc identity"
            );
            if config.notarization == Policy::Required {
                ensure!(
                    config
                        .macos_keychain_profile
                        .as_ref()
                        .is_some_and(|p| !p.trim().is_empty()),
                    "required notarization needs signing.macos-keychain-profile (a stored credential reference)"
                );
            }
        }
        "windows" => {
            ensure!(
                config.notarization == Policy::Off,
                "notarization is macOS-only"
            );
            let thumbprint = config
                .windows_thumbprint
                .as_deref()
                .context("required signing needs signing.windows-thumbprint")?;
            ensure!(
                thumbprint.len() == 40 && thumbprint.bytes().all(|b| b.is_ascii_hexdigit()),
                "certificate thumbprint must contain 40 hexadecimal characters"
            );
            ensure!(
                config
                    .windows_timestamp_url
                    .as_ref()
                    .is_some_and(|url| url.starts_with("https://") && !url.contains('@')),
                "required Windows signing needs an HTTPS timestamp URL without credentials"
            );
        }
        _ => anyhow::bail!(
            "OS code signing is not defined for Linux packages; updater signatures are outside this design"
        ),
    }
    Ok(())
}

// Calls are separated from decisions so tests can verify ordering and failure
// handling without accessing certificates or submitting notarization requests.
pub(crate) trait Runner {
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<Vec<u8>>;
}
pub(crate) struct Native;
impl Runner for Native {
    fn run(&mut self, program: &str, args: &[OsString]) -> Result<Vec<u8>> {
        process::capture(
            Command::new(
                crate::platform::find(program).context("required signing tool is missing")?,
            )
            .args(args),
        )
    }
}
fn args(values: &[&str], path: &Path) -> Vec<OsString> {
    values
        .iter()
        .map(OsString::from)
        .chain(std::iter::once(path.as_os_str().to_owned()))
        .collect()
}

pub(crate) fn finish(
    config: &Signing,
    os: &str,
    path: &Path,
    format: &str,
    runner: &mut impl Runner,
) -> Result<Verification> {
    let mut verification = Verification::default();
    if config.code_signing == Policy::Off {
        return Ok(verification);
    }
    validate(config, os, true)?;
    if os == "windows" {
        // The backend signs both the staged executable and NSIS installer using
        // the selected certificate. Verification is independent of its success.
        runner.run(
            "signtool.exe",
            &args(&["verify", "/pa", "/all", "/v"], path),
        )?;
        verification.code_signing = Outcome::Verified;
        return Ok(verification);
    }
    if format == "dmg" {
        runner.run(
            "codesign",
            &args(
                &[
                    "--force",
                    "--timestamp",
                    "--sign",
                    config.macos_identity.as_deref().context("identity")?,
                ],
                path,
            ),
        )?;
    }
    let identity = config
        .macos_identity
        .as_deref()
        .context("signing identity")?;
    let requirement = if identity.len() == 40 && identity.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("anchor apple generic and certificate leaf = H\"{identity}\"")
    } else {
        let identity = identity.replace('\\', "\\\\").replace('"', "\\\"");
        format!("anchor apple generic and certificate leaf[subject.CN] = \"{identity}\"")
    };
    runner.run(
        "codesign",
        &args(
            &[
                "--verify",
                "--deep",
                "--strict",
                "--verbose=2",
                "-R",
                &requirement,
            ],
            path,
        ),
    )?;
    verification.code_signing = Outcome::Verified;
    if config.notarization == Policy::Required {
        let temporary = tempfile::tempdir()?;
        let submission = if format == "app" {
            let zip = temporary.path().join("submission.zip");
            let mut command = args(&["-c", "-k", "--keepParent"], path);
            command.push(zip.as_os_str().to_owned());
            runner.run("ditto", &command)?;
            zip
        } else {
            path.to_owned()
        };
        let mut command = args(&["notarytool", "submit"], &submission);
        command.extend(
            [
                "--keychain-profile",
                config
                    .macos_keychain_profile
                    .as_deref()
                    .context("keychain profile")?,
                "--wait",
                "--output-format",
                "json",
            ]
            .into_iter()
            .map(OsString::from),
        );
        let response = runner.run("xcrun", &command)?;
        let response: serde_json::Value =
            serde_json::from_slice(&response).context("invalid notarization result")?;
        ensure!(
            response.get("status").and_then(|v| v.as_str()) == Some("Accepted"),
            "notarization did not return Accepted"
        );
        verification.notarization = Outcome::Verified;
        runner.run("xcrun", &args(&["stapler", "staple"], path))?;
        runner.run("xcrun", &args(&["stapler", "validate"], path))?;
        verification.stapling = Outcome::Verified;
        runner.run("codesign", &args(&["--verify", "--deep", "--strict"], path))?;
    }
    Ok(verification)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        calls: Vec<(String, Vec<OsString>)>,
        reject: bool,
    }
    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[OsString]) -> Result<Vec<u8>> {
            self.calls.push((program.into(), args.to_vec()));
            Ok(if self.reject {
                br#"{"status":"Invalid"}"#.to_vec()
            } else {
                br#"{"status":"Accepted"}"#.to_vec()
            })
        }
    }
    fn config() -> Signing {
        Signing {
            code_signing: Policy::Required,
            notarization: Policy::Required,
            macos_identity: Some("Developer ID Application: Fixture".into()),
            macos_keychain_profile: Some("fixture-reference".into()),
            ..Default::default()
        }
    }
    #[test]
    fn off_executes_nothing_and_required_needs_explicit_execution() -> Result<()> {
        let mut runner = Fake::default();
        finish(
            &Signing::default(),
            "macos",
            Path::new("App.app"),
            "app",
            &mut runner,
        )?;
        assert!(runner.calls.is_empty());
        assert!(validate(&config(), "macos", false).is_err());
        Ok(())
    }
    #[test]
    fn notarization_acceptance_and_stapling_are_independently_verified() -> Result<()> {
        let mut runner = Fake::default();
        let result = finish(&config(), "macos", Path::new("App.app"), "app", &mut runner)?;
        assert_eq!(result.stapling, Outcome::Verified);
        assert_eq!(runner.calls[0].0, "codesign");
        assert_eq!(runner.calls[1].0, "ditto");
        assert_eq!(runner.calls[4].1[1], "validate");
        let mut rejected = Fake {
            reject: true,
            ..Default::default()
        };
        assert!(
            finish(
                &config(),
                "macos",
                Path::new("App.app"),
                "app",
                &mut rejected
            )
            .is_err()
        );
        assert!(
            !rejected
                .calls
                .iter()
                .any(|(_, a)| a.first().is_some_and(|a| a == "stapler"))
        );
        Ok(())
    }
}
