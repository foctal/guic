use crate::{report, staging};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Subcommand;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Generate a key pair without replacing existing files. Never uploads keys.
    Keygen {
        /// Private key destination; the public key is written to <output>.pub.
        #[arg(long)]
        output: PathBuf,
        /// Environment variable holding the password; absent means an empty password.
        #[arg(long, default_value = "GUIC_UPDATER_PASSWORD")]
        password_env: String,
    },
    /// Copy and sign completed package artifacts and create latest.json. Never uploads.
    Prepare {
        /// One or more complete cargo-guic artifact reports for the same release.
        #[arg(long, required = true)]
        artifact_report: Vec<PathBuf>,
        /// A new directory for the feed, archives and signatures.
        #[arg(long)]
        output: PathBuf,
        /// HTTPS directory URL where the generated directory will be hosted.
        #[arg(long)]
        base_url: String,
        /// Path to the base64-encoded updater private key.
        #[arg(long)]
        private_key: PathBuf,
        /// Path to the matching base64-encoded public key embedded in the application.
        #[arg(long)]
        public_key: PathBuf,
        #[arg(long, default_value = "GUIC_UPDATER_PASSWORD")]
        password_env: String,
    },
}

pub(crate) fn run(command: &Command) -> Result<serde_json::Value> {
    match command {
        Command::Keygen {
            output,
            password_env,
        } => {
            let password = password(password_env)?;
            let keys = cargo_packager::sign::generate_key(Some(password))?;
            let public = output.with_added_extension("pub");
            ensure!(
                !output.exists() && !public.exists(),
                "key files already exist"
            );
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut private = options.open(output).context("create private key")?;
            private.write_all(keys.sk.as_bytes())?;
            private.sync_all()?;
            let mut public_file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&public)?;
            public_file.write_all(keys.pk.as_bytes())?;
            public_file.sync_all()?;
            Ok(serde_json::json!({"schema_version":1,"private_key":output,"public_key":public}))
        }
        Command::Prepare {
            artifact_report,
            output,
            base_url,
            private_key,
            public_key,
            password_env,
        } => {
            let secret =
                std::fs::read_to_string(private_key).context("read updater private key")?;
            let public = std::fs::read_to_string(public_key).context("read updater public key")?;
            prepare(
                artifact_report,
                output,
                base_url,
                secret.trim(),
                public.trim(),
                &password(password_env)?,
            )
        }
    }
}

fn password(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => Ok(String::new()),
        Err(_) => anyhow::bail!("updater password environment variable must be UTF-8"),
    }
}

fn prepare(
    reports: &[PathBuf],
    output: &Path,
    base_url: &str,
    secret: &str,
    public: &str,
    password: &str,
) -> Result<serde_json::Value> {
    let mut base = url::Url::parse(base_url)?;
    ensure!(
        base.scheme() == "https"
            && base.host_str().is_some()
            && base.username().is_empty()
            && base.password().is_none()
            && base.query().is_none()
            && base.fragment().is_none(),
        "update base URL must be HTTPS without credentials, query or fragment"
    );
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    let public = String::from_utf8(STANDARD.decode(public)?)?;
    let public = minisign::PublicKeyBox::from_string(&public)?.into_public_key()?;
    let secret = cargo_packager::sign::decode_private_key(secret, Some(password))?;
    // Check the pair before writing any artifact or feed.
    let probe = minisign::sign(
        Some(&public),
        &secret,
        std::io::Cursor::new(b"GUIC updater key check"),
        None,
        None,
    )?;
    minisign::verify(
        &public,
        &probe,
        std::io::Cursor::new(b"GUIC updater key check"),
        true,
        false,
        false,
    )?;
    let mut manifests = Vec::new();
    for path in reports {
        let value: report::ArtifactReport = report::read(path)?;
        ensure!(
            value.complete && !value.outputs.is_empty(),
            "updates require a complete nonempty artifact report"
        );
        semver::Version::parse(&value.version)?;
        manifests.push(value);
    }
    let first = manifests
        .first()
        .context("at least one artifact report is required")?;
    for item in &manifests {
        ensure!(
            item.identifier == first.identifier && item.version == first.version,
            "all updater reports must have the same application identifier and version"
        );
    }
    ensure!(!output.exists(), "updater output directory already exists");
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    // Build in isolation and publish the local feed only after all artifacts are ready.
    let temporary = tempfile::Builder::new()
        .prefix(".guic-update-")
        .tempdir_in(parent)?;
    let root = temporary.path();
    let mut platforms = BTreeMap::new();
    for item in &manifests {
        let os = crate::plan::target_os(&item.target)?;
        let format = match os {
            "macos" => "app",
            "windows" => "nsis",
            _ => "appimage",
        };
        let artifacts: Vec<_> = item.outputs.iter().filter(|a| a.format == format).collect();
        ensure!(
            artifacts.len() == 1,
            "updater requires exactly one {format} artifact per report; DMG and deb use external update workflows"
        );
        let artifact = artifacts[0];
        crate::config::reject_symlinks(&artifact.path)?;
        let (size, hash) = if format == "app" {
            ensure!(
                artifact.path.is_dir(),
                "app updater artifact must be a directory"
            );
            staging::hash_directory(&artifact.path)?
        } else {
            ensure!(
                artifact.path.is_file(),
                "updater artifact must be a regular file"
            );
            (
                artifact.path.metadata()?.len(),
                report::hash_file(&artifact.path)?,
            )
        };
        ensure!(
            size == artifact.size && hash == artifact.sha256,
            "artifact changed since packaging; regenerate the package"
        );
        let arch = item
            .target
            .split('-')
            .next()
            .context("target architecture")?;
        ensure!(
            ["aarch64", "x86_64", "i686", "armv7", "universal"].contains(&arch),
            "unsupported updater architecture"
        );
        let platform = format!("{os}-{arch}");
        let directory = root.join(&platform);
        std::fs::create_dir(&directory).context("duplicate updater platform")?;
        let filename = artifact.path.file_name().context("artifact filename")?;
        let destination = directory.join(filename);
        if format == "app" {
            crate::resources::copy_framework(&artifact.path, &destination)?;
            ensure!(
                staging::hash_directory(&destination)?.1 == artifact.sha256,
                "app changed while copying"
            );
        } else {
            std::fs::copy(&artifact.path, &destination)?;
            ensure!(
                report::hash_file(&destination)? == artifact.sha256,
                "artifact changed while copying"
            );
        }
        let payload = if format == "app" {
            let archive = destination.with_added_extension("tar.gz");
            let encoder = flate2::write::GzEncoder::new(
                File::create(&archive)?,
                flate2::Compression::default(),
            );
            let mut tar = tar::Builder::new(encoder);
            tar.follow_symlinks(false);
            tar.append_dir_all(filename, &destination)?;
            tar.into_inner()?.finish()?.sync_all()?;
            std::fs::remove_dir_all(&destination)?;
            archive
        } else {
            destination
        };
        let signature = cargo_packager::sign::sign_file_with_secret_key(&secret, &payload)?;
        let encoded = std::fs::read_to_string(&signature)?;
        let decoded = String::from_utf8(STANDARD.decode(&encoded)?)?;
        let decoded = minisign::SignatureBox::from_string(&decoded)?;
        minisign::verify(&public, &decoded, File::open(&payload)?, true, false, false)?;
        let mut url = base.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("invalid base URL"))?
            .pop_if_empty()
            .push(&platform)
            .push(
                payload
                    .file_name()
                    .and_then(|f| f.to_str())
                    .context("UTF-8 artifact name")?,
            );
        let entry = serde_json::json!({"url": url.as_str(), "signature":encoded,"format":format});
        let keys = if arch == "universal" {
            ensure!(os == "macos", "universal updates are macOS-only");
            vec!["macos-aarch64".to_owned(), "macos-x86_64".to_owned()]
        } else {
            vec![platform]
        };
        for key in keys {
            ensure!(
                platforms.insert(key, entry.clone()).is_none(),
                "duplicate update platform"
            );
        }
    }
    let manifest = serde_json::json!({"version":first.version,"platforms":platforms});
    report::atomic_new(&root.join("latest.json"), &manifest)?;
    // Reserve the destination without replacing existing output from another invocation.
    std::fs::create_dir(output).context("reserve updater output directory")?;
    let publish = (|| -> Result<()> {
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_name() != "latest.json" {
                std::fs::rename(entry.path(), output.join(entry.file_name()))?;
            }
        }
        std::fs::rename(root.join("latest.json"), output.join("latest.json"))?;
        Ok(())
    })();
    if let Err(error) = publish {
        let _ = std::fs::remove_dir_all(output);
        return Err(error);
    }
    Ok(
        serde_json::json!({"schema_version":1,"manifest":output.join("latest.json"),"version":first.version,"published":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_report(root: &Path, target: &str, format: &str) -> Result<PathBuf> {
        let artifact = root.join(format!("fixture-{target}.{format}"));
        if format == "app" {
            std::fs::create_dir_all(artifact.join("Contents/MacOS"))?;
            std::fs::write(
                artifact.join("Contents/MacOS/fixture"),
                b"executable fixture",
            )?;
        } else {
            std::fs::write(&artifact, b"installer fixture")?;
        }
        let (size, hash) = if format == "app" {
            staging::hash_directory(&artifact)?
        } else {
            (artifact.metadata()?.len(), report::hash_file(&artifact)?)
        };
        let path = root.join(format!("{target}.json"));
        report::atomic_new(
            &path,
            &serde_json::json!({
                "schema_version":1,"identifier":"org.example.fixture","version":"1.2.3",
                "build_record_id":"fixture","target":target,"profile":"release","features":[],
                "default_features":true,"requested_formats":[format],"resource_hashes":{},"tool_versions":{},
                "outputs":[{"format":format,"path":artifact,"size":size,"sha256":hash,"verification":{
                    "code_signing":"disabled","notarization":"disabled","stapling":"disabled"},
                    "kind":if format == "app" {"directory"} else {"file"}}],
                "code_signing":"disabled","notarization":"disabled","stapling":"disabled",
                "complete":true,"failed_format":null,"trial":false,"acceptance":"not-verified"
            }),
        )?;
        Ok(path)
    }

    #[test]
    fn update_feed_checks_integrity_and_matches_the_runtime_protocol() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        let keys = cargo_packager::sign::generate_key(Some(String::new()))?;
        let reports = [
            artifact_report(&root, "universal-apple-darwin", "app")?,
            artifact_report(&root, "x86_64-pc-windows-msvc", "nsis")?,
            artifact_report(&root, "x86_64-unknown-linux-gnu", "appimage")?,
        ];
        let output = root.join("updates");
        prepare(
            &reports,
            &output,
            "https://example.com/releases/1.2.3/",
            &keys.sk,
            &keys.pk,
            "",
        )?;
        let feed: serde_json::Value =
            serde_json::from_reader(File::open(output.join("latest.json"))?)?;
        assert_eq!(feed["platforms"].as_object().context("platforms")?.len(), 4);
        assert_eq!(
            feed["platforms"]["macos-aarch64"],
            feed["platforms"]["macos-x86_64"]
        );
        let release: cargo_packager_updater::RemoteRelease = serde_json::from_value(feed.clone())?;
        assert_eq!(release.version.to_string(), "1.2.3");
        let mac = &feed["platforms"]["macos-aarch64"];
        let url = url::Url::parse(mac["url"].as_str().context("url")?)?;
        let archive = output.join("macos-universal").join(
            url.path_segments()
                .context("segments")?
                .next_back()
                .context("filename")?,
        );
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive)?));
        assert!(tar.entries()?.any(|entry| entry.is_ok_and(|e| {
            e.path()
                .is_ok_and(|p| p.ends_with("Contents/MacOS/fixture"))
        })));
        assert!(
            prepare(
                &reports,
                &output,
                "https://example.com/",
                &keys.sk,
                &keys.pk,
                ""
            )
            .is_err()
        );
        let bad_output = root.join("bad");
        assert!(
            prepare(
                &reports,
                &bad_output,
                "http://example.com/",
                &keys.sk,
                &keys.pk,
                ""
            )
            .is_err()
        );
        assert!(!bad_output.exists());
        std::fs::write(
            root.join("fixture-x86_64-unknown-linux-gnu.appimage"),
            b"tampered",
        )?;
        assert!(
            prepare(
                &reports,
                &bad_output,
                "https://example.com/",
                &keys.sk,
                &keys.pk,
                ""
            )
            .is_err()
        );
        assert!(!bad_output.exists());
        Ok(())
    }

    #[test]
    fn runtime_download_verifies_signatures_before_installation() -> Result<()> {
        use std::{io::Read, net::TcpListener, time::Duration};
        let dir = tempfile::tempdir()?;
        let root = dir.path().canonicalize()?;
        let keys = cargo_packager::sign::generate_key(Some(String::new()))?;
        let report = artifact_report(&root, "x86_64-unknown-linux-gnu", "appimage")?;
        let output = root.join("updates");
        prepare(
            &[report],
            &output,
            "https://example.com/",
            &keys.sk,
            &keys.pk,
            "",
        )?;
        let original: serde_json::Value =
            serde_json::from_reader(File::open(output.join("latest.json"))?)?;
        for tampered in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?;
            let mut feed = original.clone();
            feed["platforms"]["linux-x86_64"]["url"] = format!("http://{address}/artifact").into();
            let feed = serde_json::to_vec(&feed)?;
            let payload = if tampered {
                b"tampered fixture".to_vec()
            } else {
                b"installer fixture".to_vec()
            };
            let server = std::thread::spawn(move || -> std::io::Result<()> {
                for response in [feed, payload] {
                    let (mut socket, _) = listener.accept()?;
                    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
                    let mut request = [0; 4096];
                    let _ = socket.read(&mut request)?;
                    write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response.len()
                    )?;
                    socket.write_all(&response)?;
                }
                Ok(())
            });
            let updater = cargo_packager_updater::UpdaterBuilder::new(
                "1.0.0".parse()?,
                cargo_packager_updater::Config {
                    endpoints: vec![format!("http://{address}/latest.json").parse()?],
                    pubkey: keys.pk.clone(),
                    ..Default::default()
                },
            )
            .target("linux-x86_64")
            .timeout(Duration::from_secs(5))
            .build()?;
            let update = updater.check()?.context("new release")?;
            let bytes = update.download();
            server.join().expect("test HTTP server")?;
            if tampered {
                assert!(bytes.is_err());
            } else {
                assert_eq!(bytes?, b"installer fixture");
            }
        }
        Ok(())
    }
}
