use std::{fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let themes_dir = manifest_dir.join("themes");

    for entry in fs::read_dir(&themes_dir).expect("theme directory should exist") {
        let entry = entry.expect("theme directory entry should load");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());
        let contents = fs::read_to_string(&path).expect("theme file should be readable");
        serde_json::from_str::<serde_json::Value>(&contents).unwrap_or_else(|error| {
            panic!("invalid built-in theme JSON at {}: {error}", path.display())
        });
    }
}
