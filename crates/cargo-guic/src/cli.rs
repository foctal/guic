use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cargo guic",
    version,
    about = "Build and inspect GUIC applications"
)]
pub(crate) struct Cli {
    /// Reserve stdout for versioned JSON reports. Child output goes to stderr.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Create Guic.toml without replacing existing files.
    Init(Init),
    /// Build and run; optionally watch files and restart on changes.
    Dev {
        #[command(flatten)]
        selectors: Selectors,
        /// Rebuild and restart after source or resource changes.
        #[arg(long, conflicts_with = "no_watch")]
        watch: bool,
        /// Run once (the default).
        #[arg(long)]
        no_watch: bool,
        /// Additional files or directories to watch, including external dependencies.
        #[arg(long, requires = "watch")]
        watch_path: Vec<PathBuf>,
        /// Files or directories excluded from watching.
        #[arg(long, requires = "watch")]
        watch_ignore: Vec<PathBuf>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Build a release executable and, by default, package it.
    Build {
        #[command(flatten)]
        selectors: Selectors,
        #[arg(long)]
        no_bundle: bool,
        /// Explicitly execute the configured signing and notarization policies.
        #[arg(long, conflicts_with = "no_bundle")]
        execute_signing: bool,
    },
    /// Package a verified immutable build snapshot without compiling.
    Bundle {
        #[command(flatten)]
        selectors: Selectors,
        #[arg(long)]
        build_record: Option<PathBuf>,
        /// Explicitly execute the configured signing and notarization policies.
        #[arg(long)]
        execute_signing: bool,
    },
    /// Generate candidate packages for manual acceptance; never certifies distribution support.
    PackageTrial {
        #[command(flatten)]
        selectors: Selectors,
        #[arg(long)]
        build_record: PathBuf,
        /// Explicitly execute the configured signing and notarization policies.
        #[arg(long)]
        execute_signing: bool,
    },
    /// Inspect resolved configuration without executing hooks or builds.
    Inspect(Selectors),
    /// Diagnose prerequisites without installing tools or executing hooks.
    Doctor {
        #[command(flatten)]
        selectors: Selectors,
        /// Inspect imports and ABI requirements of a verified build snapshot.
        #[arg(long)]
        build_record: Option<PathBuf>,
    },
    /// Print the versioned Guic.toml JSON Schema.
    Schema,
    /// Generate update signing keys or prepare signed update feeds locally.
    Updater {
        #[command(subcommand)]
        command: crate::updater::Command,
    },
    #[command(name = "__worker", hide = true)]
    Worker { request: PathBuf, response: PathBuf },
}

impl Command {
    pub fn selectors(&self) -> Option<&Selectors> {
        match self {
            Self::Dev { selectors, .. }
            | Self::Build { selectors, .. }
            | Self::Bundle { selectors, .. }
            | Self::PackageTrial { selectors, .. }
            | Self::Inspect(selectors)
            | Self::Doctor { selectors, .. } => Some(selectors),
            _ => None,
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub(crate) struct Selectors {
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,
    #[arg(short, long)]
    pub package: Option<String>,
    #[arg(long)]
    pub bin: Option<String>,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub features: Option<Vec<String>>,
    #[arg(long)]
    pub no_default_features: bool,
    #[arg(long, value_delimiter = ',', num_args = 1)]
    pub formats: Option<Vec<String>>,
    /// Permit Cargo to update the application lockfile.
    #[arg(long)]
    pub unlocked: bool,
}

#[derive(Args)]
pub(crate) struct Init {
    #[command(flatten)]
    pub selectors: Selectors,
    /// Stable reverse-domain application identifier (never inferred).
    #[arg(long)]
    pub identifier: String,
    #[arg(long)]
    pub product_name: Option<String>,
}
