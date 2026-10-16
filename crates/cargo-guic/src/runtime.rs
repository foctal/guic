use crate::{doctor::Check, plan::BuildPlan};
use std::{collections::BTreeSet, path::Path, process::Command};

fn check(name: &str, passed: Option<bool>, required: bool, detail: impl Into<String>) -> Check {
    Check {
        name: name.into(),
        passed,
        required,
        detail: detail.into(),
    }
}
fn output(tool: &str, args: &[&str]) -> anyhow::Result<String> {
    let path = crate::platform::find(tool)
        .ok_or_else(|| anyhow::anyhow!("missing diagnostic tool: {tool}"))?;
    Ok(
        String::from_utf8_lossy(&crate::process::capture(Command::new(path).args(args))?)
            .into_owned(),
    )
}

pub(crate) fn diagnose(plan: &BuildPlan, executable: Option<&Path>) -> Vec<Check> {
    let mut checks = Vec::new();
    if plan.os == "linux" {
        let mut packages = plan.config.runtime.pkg_config.clone();
        packages.extend(
            [
                "fontconfig",
                "x11",
                "xcb",
                "xkbcommon",
                "wayland-client",
                "openssl",
                "alsa",
            ]
            .map(str::to_owned),
        );
        if plan.config.runtime.webview {
            packages.extend(["gtk+-3.0".into(), "webkit2gtk-4.1".into()]);
        }
        packages.sort();
        packages.dedup();
        for package in packages {
            let result = output("pkg-config", &["--modversion", &package]);
            checks.push(check(
                &format!("build-package:{package}"),
                Some(result.is_ok()),
                true,
                result.unwrap_or_else(|e| {
                    format!("{e}; install the development package providing {package}.pc")
                }),
            ));
        }
        checks.push(check("display-session", Some(std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()), false,
            "GUI launch needs X11 or Wayland; the WebView fixture requires X11/XWayland even in a Wayland session"));
        if plan.formats.iter().any(|f| f == "appimage") {
            checks.push(check("appimage-fuse", Some(Path::new("/dev/fuse").exists()), false,
                "Mounted AppImages require a working FUSE setup; also validate --appimage-extract and AppRun on a clean host"));
        }
    }
    if plan.config.runtime.webview {
        let result = match plan.os.as_str() {
            "windows" => output(
                "powershell.exe",
                &[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"$paths = @('HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}','HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}','HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'); $v = @(Get-ItemProperty -Path $paths -ErrorAction SilentlyContinue | Where-Object { $_.pv -match '^\d+\.\d+\.\d+\.\d+$' -and $_.pv -ne '0.0.0.0' } | Select-Object -ExpandProperty pv); if ($v.Count -eq 0) { exit 1 }; $v"#,
                ],
            ),
            "macos" => {
                if Path::new("/System/Library/Frameworks/WebKit.framework").exists() {
                    Ok("System WebKit framework exists".into())
                } else {
                    Err(anyhow::anyhow!("system WebKit framework is missing"))
                }
            }
            _ => output("pkg-config", &["--modversion", "webkit2gtk-4.1"]),
        };
        checks.push(check("webview-runtime", Some(result.is_ok()), true,
            result.unwrap_or_else(|e| format!("{e}; provision the native WebView runtime manually. No bootstrapper is downloaded or executed"))));
    }
    let imported = if let Some(executable) = executable {
        let binary = executable.to_string_lossy();
        match plan.os.as_str() {
            "windows" => {
                output("dumpbin.exe", &["/DEPENDENTS", &binary]).map(|s| windows_imports(&s))
            }
            "macos" => output("otool", &["-L", &binary]).map(|s| macos_imports(&s)),
            _ => output("readelf", &["--dynamic", &binary]).map(|s| elf_imports(&s)),
        }
    } else {
        checks.push(check("binary-runtime-dependencies", None, false,
            "Pass --build-record to inspect a verified snapshot's imports and ABI requirements without executing it"));
        Ok(BTreeSet::new())
    };
    let mut libraries = plan
        .config
        .runtime
        .dynamic_libraries
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    match imported {
        Ok(imports) => {
            libraries.extend(imports);
            if executable.is_some() {
                checks.push(check("binary-import-inspection", Some(true), true, "Inspected import table; explicit dynamic requirements are checked separately from build packages"));
            }
        }
        Err(e) => checks.push(check(
            "binary-import-inspection",
            Some(false),
            true,
            e.to_string(),
        )),
    }
    let cache = if plan.os == "linux" {
        output("ldconfig", &["-p"]).ok()
    } else {
        None
    };
    let resources = crate::resources::collect(&plan.config.bundle.resources).unwrap_or_default();
    for library in libraries {
        let bundled = resources.iter().any(|r| {
            r.destination
                .to_string_lossy()
                .eq_ignore_ascii_case(&library)
        });
        let (found, detail): (Option<bool>, String) = match plan.os.as_str() {
            "windows" => {
                let lower = library.to_ascii_lowercase();
                if lower.starts_with("api-ms-win-") || lower.starts_with("ext-ms-win-") {
                    (
                        None,
                        "Windows API-set contract; verify supported Windows baseline".into(),
                    )
                } else {
                    let system = std::env::var_os("SystemRoot")
                        .map(|root| {
                            std::path::PathBuf::from(root)
                                .join("System32")
                                .join(&library)
                                .is_file()
                        })
                        .unwrap_or(false);
                    (Some(bundled || system), if bundled { "Explicitly bundled beside the executable" } else if system { "Present in host System32; clean-host redistributable policy still needs acceptance" } else { "Missing: bundle an authorized DLL at the install root or require its runtime installer" }.into())
                }
            }
            "macos" => {
                if library.starts_with("/System/Library/") || library.starts_with("/usr/lib/") {
                    (None, "System library may be provided by the dyld shared cache; verify deployment baseline".into())
                } else if library.starts_with('@') {
                    let framework = plan
                        .config
                        .backend
                        .cargo_packager
                        .macos_frameworks
                        .iter()
                        .any(|p| {
                            p.file_name().is_some_and(|name| {
                                library.contains(&name.to_string_lossy().to_string())
                            })
                        });
                    (None, if framework { "Configured framework; inspect loader rpaths and installed launch" } else { "Loader-relative dependency; inspect otool -l LC_RPATH and installed launch" }.into())
                } else {
                    (
                        Some(Path::new(&library).is_file()),
                        "Absolute dependency: verify availability on the destination system".into(),
                    )
                }
            }
            _ => {
                let found = cache.as_ref().map(|text| {
                    text.lines()
                        .any(|line| line.split_whitespace().next() == Some(library.as_str()))
                });
                (found, if bundled { "Resource supplied, but resource packaging alone does not change the dynamic loader search path" } else { "Checked host ldconfig cache; declare required Debian packages and verify clean-host/AppImage loading" }.into())
            }
        };
        checks.push(check(
            &format!("runtime-library:{library}"),
            found,
            true,
            detail,
        ));
    }
    let Some(executable) = executable else {
        return checks;
    };
    let binary = executable.to_string_lossy();
    if plan.os == "linux" {
        match output("readelf", &["--version-info", &binary]) {
            Ok(text) => {
                let required = glibc_versions(&text).into_iter().max();
                let baseline = plan.config.runtime.glibc_max.as_deref().map(version);
                let passed = baseline
                    .as_ref()
                    .map(|limit| required.as_ref().is_none_or(|v| v <= limit));
                checks.push(check("glibc-baseline", passed, baseline.is_some(), format!("Maximum imported GLIBC version: {}; configured maximum: {}. Build on the oldest supported distribution; packaging cannot lower ABI requirements", required.map(|v| v.iter().map(u32::to_string).collect::<Vec<_>>().join(".")).unwrap_or_else(|| "none".into()), plan.config.runtime.glibc_max.as_deref().unwrap_or("not specified"))));
            }
            Err(e) => checks.push(check(
                "glibc-baseline",
                Some(false),
                plan.config.runtime.glibc_max.is_some(),
                e.to_string(),
            )),
        }
    }
    if plan.os == "macos" {
        match output("otool", &["-l", &binary]) {
            Ok(text) => checks.push(check("mach-o-load-commands", None, false, text)),
            Err(e) => checks.push(check(
                "mach-o-load-commands",
                Some(false),
                false,
                e.to_string(),
            )),
        }
    }
    checks
}

fn version(text: &str) -> Vec<u32> {
    let mut parts: Vec<u32> = text.split('.').filter_map(|n| n.parse().ok()).collect();
    while parts.len() > 2 && parts.last() == Some(&0) {
        parts.pop();
    }
    parts
}
fn glibc_versions(text: &str) -> BTreeSet<Vec<u32>> {
    text.split("GLIBC_")
        .skip(1)
        .filter_map(|part| {
            let numeric: String = part
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let parsed = version(&numeric);
            (parsed.len() >= 2).then_some(parsed)
        })
        .collect()
}
fn elf_imports(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| line.contains("(NEEDED)"))
        .filter_map(|line| {
            line.split_once('[')?
                .1
                .split_once(']')
                .map(|(name, _)| name.to_owned())
        })
        .collect()
}
fn windows_imports(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|s| s.to_ascii_lowercase().ends_with(".dll") && !s.contains(char::is_whitespace))
        .map(str::to_owned)
        .collect()
}
fn macos_imports(text: &str) -> BTreeSet<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            line.trim()
                .split_once(" (compatibility version")
                .map(|(name, _)| name.to_owned())
        })
        .collect()
}

pub(crate) fn validate_binary_policy(plan: &BuildPlan, executable: &Path) -> anyhow::Result<()> {
    if plan.os == "linux"
        && let Some(limit) = &plan.config.runtime.glibc_max
    {
        let text = output(
            "readelf",
            &["--version-info", &executable.to_string_lossy()],
        )?;
        anyhow::ensure!(
            glibc_versions(&text).iter().all(|v| v <= &version(limit)),
            "executable exceeds runtime.glibc-max; rebuild on the oldest supported Linux system"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_native_import_tables_without_running_binaries() {
        assert_eq!(
            elf_imports("0x1 (NEEDED) Shared library: [libc.so.6]\n0x1 (RUNPATH) [ignored]"),
            BTreeSet::from(["libc.so.6".into()])
        );
        assert_eq!(
            windows_imports("Dependencies:\n KERNEL32.dll\n VCRUNTIME140.dll\nSummary"),
            BTreeSet::from(["KERNEL32.dll".into(), "VCRUNTIME140.dll".into()])
        );
        assert_eq!(
            macos_imports(
                "app:\n @rpath/My Framework.framework/My Framework (compatibility version 1.0.0, current version 1.0.0)"
            ),
            BTreeSet::from(["@rpath/My Framework.framework/My Framework".into()])
        );
        assert_eq!(
            glibc_versions("GLIBC_2.9 GLIBC_2.34 GLIBC_PRIVATE")
                .into_iter()
                .max(),
            Some(vec![2, 34])
        );
    }
}
