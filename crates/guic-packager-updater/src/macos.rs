// Copyright 2026 GUIC contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    ffi::CString,
    fs,
    io::{self, Cursor},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
    process::Command,
};

use flate2::read::GzDecoder;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(super) fn install(target: &Path, bytes: Vec<u8>) -> crate::Result<()> {
    install_with_authorization(target, bytes, |command| {
        let script = format!(
            "do shell script {} with administrator privileges",
            applescript_string(command)
        );
        let output = Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "The update was not authorized or could not be installed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ))
        }
    })?;
    Ok(())
}

fn install_with_authorization(
    target: &Path,
    bytes: Vec<u8>,
    authorize: impl FnOnce(&str) -> io::Result<()>,
) -> io::Result<()> {
    if target.extension() != Some(std::ffi::OsStr::new("app")) {
        return Err(invalid("The installation target must be an .app bundle"));
    }
    if !fs::symlink_metadata(target)?.file_type().is_dir() {
        return Err(invalid(
            "The installed bundle must be a directory, not a symbolic link",
        ));
    }
    let target = target.canonicalize()?;
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Missing application parent"))?;
    // Prefer the destination's volume. A protected parent is handled only after
    // extraction succeeds, by the user-authorized installer below.
    let (stage, protected_parent) = match tempfile::Builder::new()
        .prefix(".packager-update-")
        .tempdir_in(parent)
    {
        Ok(stage) => (stage, false),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            (tempfile::tempdir()?, true)
        }
        Err(error) => return Err(error),
    };
    let bundle = extract(&bytes, stage.path())?;
    fs::set_permissions(&bundle, fs::metadata(&target)?.permissions())?;
    if !protected_parent {
        match swap(&target, &bundle) {
            Ok(()) => {
                let old_bundle = stage.path().to_path_buf();
                if let Err(error) = stage.close() {
                    log::warn!(
                        "Update installed; cleanup of {} failed: {error}",
                        old_bundle.display()
                    );
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {}
            Err(error) => return Err(error),
        }
    }
    // No target mutation has occurred. Cancellation must leave it unchanged.
    authorize(&privileged_command(&target, &bundle)?)
}

// Extract through tar's containment checks instead of unpacking a manually
// joined destination. Restrict archive members and links to one app bundle.
fn extract(bytes: &[u8], stage: &Path) -> io::Result<PathBuf> {
    let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let mut root: Option<PathBuf> = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let components = normal_components(&path)?;
        if components.is_empty() {
            if entry.header().entry_type().is_dir() {
                continue;
            }
            return Err(invalid("Empty archive member"));
        }
        let member_root = PathBuf::from(&components[0]);
        if member_root.extension() != Some(std::ffi::OsStr::new("app")) {
            return Err(invalid("The archive must contain one .app bundle"));
        }
        if let Some(root) = &root {
            if root != &member_root {
                return Err(invalid("Multiple archive roots"));
            }
        } else {
            root = Some(member_root.clone());
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
            return Err(invalid("Unsupported archive member type"));
        }
        if components.len() == 1 && !kind.is_dir() {
            return Err(invalid("The bundle root must be a directory"));
        }
        if kind.is_symlink() || kind.is_hard_link() {
            let link = entry
                .link_name()?
                .ok_or_else(|| invalid("Missing link target"))?;
            let base = if kind.is_symlink() {
                path.parent().unwrap_or(Path::new(""))
            } else {
                Path::new("")
            };
            let resolved = resolve_link(base, &link)?;
            if resolved.components().next() != member_root.components().next() {
                return Err(invalid("Archive link escapes the bundle"));
            }
        }
        if !entry.unpack_in(stage)? {
            return Err(invalid("Archive member escapes staging"));
        }
    }
    let bundle = stage.join(root.ok_or_else(|| invalid("Empty update archive"))?);
    if !bundle.join("Contents/Info.plist").is_file() || !bundle.join("Contents/MacOS").is_dir() {
        return Err(invalid(
            "The update is missing its application bundle contents",
        ));
    }
    Ok(bundle)
}

fn normal_components(path: &Path) -> io::Result<Vec<std::ffi::OsString>> {
    path.components()
        .filter_map(|component| match component {
            Component::CurDir => None,
            Component::Normal(name) => Some(Ok(name.to_owned())),
            _ => Some(Err(invalid("Unsafe archive path"))),
        })
        .collect()
}

fn resolve_link(base: &Path, link: &Path) -> io::Result<PathBuf> {
    let mut result = base.to_path_buf();
    for component in link.components() {
        match component {
            Component::Normal(name) => result.push(name),
            Component::CurDir => {}
            Component::ParentDir if result.pop() => {}
            _ => return Err(invalid("Unsafe archive link")),
        }
    }
    Ok(result)
}

fn swap(target: &Path, staged: &Path) -> io::Result<()> {
    let from = CString::new(staged.as_os_str().as_bytes())?;
    let to = CString::new(target.as_os_str().as_bytes())?;
    // SAFETY: both pointers refer to live NUL-terminated path strings. The
    // syscall atomically exchanges directory entries and does not retain them.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_SWAP) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

// The privileged route uses the same atomic operation through macOS's built-in
// JavaScript bridge. Never fall back to deleting or moving the old app first.
const SWAP_JXA: &str = r#"function run(argv) {
    ObjC.import("stdio");
    return $.renamex_np(argv[0], argv[1], 2) === 0 ? "swapped" : "failed";
}"#;

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn privileged_command(target: &Path, source: &Path) -> io::Result<String> {
    let parent = target
        .parent()
        .ok_or_else(|| invalid("Missing application parent"))?;
    let argument = |path: &Path| {
        path.to_str()
            .map(shell_quote)
            .ok_or_else(|| invalid("Authorization requires UTF-8 paths"))
    };
    // A private, privileged staging directory on the target volume prevents
    // cross-device replacement. Failed/ambiguous swap execution keeps staging
    // for recovery; only a confirmed result permits cleanup.
    let script = format!(
        r#"set -eu
source=$1
target=$2
parent=$3
[ -d "$target" ] && [ ! -L "$target" ] || exit 1
owner=$(/usr/bin/stat -f '%u:%g' "$target")
mode=$(/usr/bin/stat -f '%Lp' "$target")
stage=$(/usr/bin/mktemp -d "$parent/.packager-update.XXXXXXXX")
if ! /usr/bin/ditto "$source" "$stage/new.app" || ! /usr/sbin/chown -Rh "$owner" "$stage/new.app" || ! /bin/chmod "$mode" "$stage/new.app"; then
    /bin/rm -rf "$stage"
    exit 1
fi
if result=$(/usr/bin/osascript -l JavaScript -e {} "$stage/new.app" "$target"); then
    case "$result" in
        swapped) /bin/rm -rf "$stage" || :; exit 0;;
        failed) /bin/rm -rf "$stage"; echo 'Atomic application replacement failed; the installed application was not changed.' >&2; exit 1;;
    esac
fi
printf 'Application replacement status is unknown; recovery files retained at: %s\n' "$stage" >&2
exit 1"#,
        shell_quote(SWAP_JXA)
    );
    Ok(format!(
        "/bin/sh -c {} packager-update {} {} {}",
        shell_quote(&script),
        argument(source)?,
        argument(target)?,
        argument(parent)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn archive(entries: &[(&str, &[u8], u8, Option<&str>)]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for (path, data, kind, link) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::new(*kind));
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            let raw = path.as_bytes();
            header.as_mut_bytes()[..raw.len()].copy_from_slice(raw);
            if let Some(link) = link {
                header.set_link_name(link).unwrap();
            }
            header.set_cksum();
            tar.append(&header, *data).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    fn valid_archive() -> Vec<u8> {
        archive(&[
            ("New.app", b"", b'5', None),
            ("New.app/Contents/Info.plist", b"fixture", b'0', None),
            ("New.app/Contents/MacOS/main", b"new", b'0', None),
            (
                "New.app/Contents/Resources/link",
                b"",
                b'2',
                Some("../MacOS/main"),
            ),
        ])
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::Builder::new()
            .prefix("it's \"quoted\" \\ $(touch injected) `name`\n")
            .tempdir()
            .unwrap();
        let target = dir.path().join("Alice's \"Application\".app");
        fs::create_dir_all(target.join("Contents/MacOS")).unwrap();
        fs::write(target.join("Contents/MacOS/main"), "old").unwrap();
        (dir, target)
    }

    #[test]
    fn ordinary_install_handles_paths_and_preserves_bundle_mode() {
        let (dir, target) = fixture();
        install_with_authorization(&target, valid_archive(), |_| {
            panic!("unexpected authorization")
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(target.join("Contents/Resources/link")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(!dir.path().join("injected").exists());
    }

    #[test]
    fn private_bundle_permissions_are_preserved() {
        let (_dir, target) = fixture();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        install_with_authorization(&target, valid_archive(), |_| {
            panic!("unexpected authorization")
        })
        .unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn non_bundle_install_target_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("sentinel"), "keep").unwrap();
        assert!(
            install_with_authorization(dir.path(), valid_archive(), |_| panic!(
                "unexpected authorization"
            ))
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("sentinel")).unwrap(),
            "keep"
        );
    }

    #[test]
    fn malformed_archives_leave_original_untouched() {
        let (dir, target) = fixture();
        let invalid_archives = [
            archive(&[]),
            archive(&[("New.app/file", b"missing bundle contents", b'0', None)]),
            archive(&[
                ("New.app/a", b"a", b'0', None),
                ("Other.app/b", b"b", b'0', None),
            ]),
            archive(&[("../outside", b"escape", b'0', None)]),
            archive(&[("/tmp/absolute", b"escape", b'0', None)]),
            archive(&[("New.app/Contents/link", b"", b'2', Some("../../outside"))]),
            archive(&[("New.app/Contents/link", b"", b'2', Some("/tmp/outside"))]),
            archive(&[("New.app/Contents/link", b"", b'1', Some("Other.app/file"))]),
            archive(&[("New.app/fifo", b"", b'6', None)]),
        ];
        for bytes in invalid_archives {
            assert!(install_with_authorization(&target, bytes, |_| panic!(
                "invalid update must not prompt"
            ))
            .is_err());
            assert_eq!(
                fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
                "old"
            );
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn symlink_install_target_is_rejected() {
        let (dir, target) = fixture();
        let alias = dir.path().join("Alias.app");
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        assert!(
            install_with_authorization(&alias, valid_archive(), |_| panic!(
                "unexpected authorization"
            ))
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "old"
        );
    }

    struct RestoreMode(PathBuf);
    impl Drop for RestoreMode {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    fn cancellation_preserves_the_installed_application() {
        let (dir, target) = fixture();
        let restore = RestoreMode(dir.path().to_path_buf());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let mut prompted = false;
        let result = install_with_authorization(&target, valid_archive(), |_| {
            prompted = true;
            assert_eq!(
                fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
                "old"
            );
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "User cancelled",
            ))
        });
        drop(restore);
        assert!(prompted);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "old"
        );
    }

    #[test]
    fn install_stages_on_the_destination_volume() {
        use std::os::unix::fs::MetadataExt;
        let root = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let target = root.path().join("External.app");
        fs::create_dir_all(target.join("Contents/MacOS")).unwrap();
        fs::write(target.join("Contents/MacOS/main"), "old").unwrap();
        let other_volume = tempfile::tempdir().unwrap();
        if root.path().metadata().unwrap().dev() != other_volume.path().metadata().unwrap().dev() {
            let staged = extract(&valid_archive(), other_volume.path()).unwrap();
            assert_eq!(
                swap(&target, &staged).unwrap_err().raw_os_error(),
                Some(libc::EXDEV)
            );
            assert_eq!(
                fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
                "old"
            );
        }
        install_with_authorization(&target, valid_archive(), |_| {
            panic!("unexpected authorization")
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "new"
        );
    }

    #[test]
    fn failed_swap_never_moves_the_original() {
        let (dir, target) = fixture();
        assert!(swap(&target, &dir.path().join("missing")).is_err());
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "old"
        );
    }

    #[test]
    fn authorization_command_handles_shell_and_applescript_characters() {
        for through_applescript in [false, true] {
            let (dir, target) = fixture();
            let stage = tempfile::tempdir().unwrap();
            let bundle = extract(&valid_archive(), stage.path()).unwrap();
            let command = privileged_command(&target, &bundle).unwrap();
            let output = if through_applescript {
                Command::new("/usr/bin/osascript")
                    .args([
                        "-e",
                        &format!("do shell script {}", applescript_string(&command)),
                    ])
                    .current_dir(dir.path())
                    .output()
                    .unwrap()
            } else {
                Command::new("/bin/sh")
                    .args(["-c", &command])
                    .current_dir(dir.path())
                    .output()
                    .unwrap()
            };
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
                "new"
            );
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o755
            );
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            assert!(!dir.path().join("injected").exists());
        }
    }

    #[test]
    fn authorization_stages_the_copy_on_the_target_volume() {
        let root = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let target = root.path().join("External.app");
        fs::create_dir_all(target.join("Contents/MacOS")).unwrap();
        fs::write(target.join("Contents/MacOS/main"), "old").unwrap();
        let source = tempfile::tempdir().unwrap();
        let bundle = extract(&valid_archive(), source.path()).unwrap();
        let command = privileged_command(&target, &bundle).unwrap();
        let output = Command::new("/bin/sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "new"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn authorization_copy_failure_preserves_original_and_cleans_staging() {
        let (dir, target) = fixture();
        let command = privileged_command(&target, &dir.path().join("missing")).unwrap();
        let output = Command::new("/bin/sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "old"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    #[ignore = "Interactive macOS authorization cancellation; only modifies a temporary fixture"]
    fn native_authorization_cancellation() {
        let (dir, target) = fixture();
        let restore = RestoreMode(dir.path().to_path_buf());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        eprintln!(
            "Cancel the macOS authorization dialog. Fixture: {}",
            target.display()
        );
        let result = install(&target, valid_archive());
        drop(restore);
        assert!(result.is_err(), "This test expects the user to cancel");
        assert_eq!(
            fs::read_to_string(target.join("Contents/MacOS/main")).unwrap(),
            "old"
        );
    }
}
