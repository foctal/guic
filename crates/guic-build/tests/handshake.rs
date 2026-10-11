use std::{fs, process::Command};

#[test]
fn cargo_build_script_acknowledges_and_refreshes_requests() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fs::create_dir(root.join("src"))?;
    let helper = toml::Value::String(env!("CARGO_MANIFEST_DIR").into()).to_string();
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname='guic-handshake-fixture'\nversion='1.2.3'\nedition='2024'\n[workspace]\n[build-dependencies]\nguic-build={{path={helper}}}\n"
        ),
    )?;
    fs::write(
        root.join("build.rs"),
        "fn main() { guic_build::build().expect(\"GUIC build request\"); }",
    )?;
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"{}\", option_env!(\"GUIC_BUILD_REQUEST_HASH\").unwrap_or(\"ordinary-cargo\")); }",
    )?;
    let request = guic_build::Request {
        schema_version: 1,
        package: "guic-handshake-fixture".into(),
        bin: "guic-handshake-fixture".into(),
        product_name: "Fixture".into(),
        version: "1.2.3".into(),
        windows: Default::default(),
    };
    for hash in [Some("a".repeat(64)), Some("b".repeat(64)), None] {
        let mut cargo = Command::new("cargo");
        cargo
            .current_dir(root)
            .args(["build", "--offline", "--message-format=json"])
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env_remove("CARGO_BUILD_TARGET")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("GUIC_BUILD_REQUEST")
            .env_remove("GUIC_BUILD_REQUEST_HASH");
        if let Some(hash) = &hash {
            cargo
                .env("GUIC_BUILD_REQUEST", serde_json::to_string(&request)?)
                .env("GUIC_BUILD_REQUEST_HASH", hash);
        }
        let output = cargo.output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let messages = String::from_utf8(output.stdout)?;
        let mut executable = None;
        for line in messages.lines() {
            let message: serde_json::Value = serde_json::from_str(line)?;
            if message["reason"] == "compiler-artifact"
                && let Some(path) = message["executable"].as_str()
            {
                executable = Some(path.to_owned());
            }
        }
        let output = Command::new(executable.expect("Cargo executable artifact")).output()?;
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)?.trim(),
            hash.as_deref().unwrap_or("ordinary-cargo")
        );
    }
    Ok(())
}
