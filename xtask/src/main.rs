//! Repository automation helpers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context, Result, bail};
use guic_tokens::ThemeSchema;
use std::{env, fs, path::PathBuf};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("schema") => run_schema(args.any(|arg| arg == "--check")),
        Some(other) => bail!("unknown xtask command: {other}"),
        None => bail!("expected an xtask command"),
    }
}

fn run_schema(check: bool) -> Result<()> {
    let schema_path = workspace_root()?.join("docs/theme.schema.json");
    let schema = guic_tokens::theme_schema();
    write_schema(schema_path, &schema, check)
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(PathBuf::from)
        .context("xtask should live one level below the workspace root")
}

fn write_schema(path: PathBuf, schema: &ThemeSchema, check: bool) -> Result<()> {
    let rendered = serde_json::to_string_pretty(schema)? + "\n";

    if check {
        let current = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        // Git may check out the schema with CRLF line endings on Windows.
        if current.replace("\r\n", "\n") != rendered {
            bail!("theme schema is out of date: {}", path.display());
        }
        return Ok(());
    }

    fs::write(&path, rendered).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_check_accepts_checkout_line_endings_but_rejects_stale_content() {
        let path = env::temp_dir().join(format!("guic-schema-check-{}.json", std::process::id()));
        let schema = guic_tokens::theme_schema();
        let rendered = serde_json::to_string_pretty(&schema).expect("serialize schema") + "\n";

        for contents in [rendered.clone(), rendered.replace('\n', "\r\n")] {
            fs::write(&path, &contents).expect("write schema fixture");
            write_schema(path.clone(), &schema, true).expect("equivalent schema should pass");
            assert_eq!(fs::read_to_string(&path).expect("read fixture"), contents);
        }

        fs::write(&path, "{}\r\n").expect("write stale schema fixture");
        let result = write_schema(path.clone(), &schema, true);
        fs::remove_file(&path).expect("remove schema fixture");
        assert!(result.is_err(), "changed schema content must still fail");
    }
}
