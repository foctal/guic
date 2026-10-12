mod backend;
mod build;
mod cli;
mod config;
mod dev;
mod doctor;
mod plan;
mod platform;
mod process;
mod project;
mod report;
mod resources;
mod runtime;
mod signing;
mod staging;
mod updater;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};

fn run(cli: &Cli) -> Result<serde_json::Value> {
    match &cli.command {
        Command::Updater { command } => return updater::run(command),
        Command::Schema => return Ok(serde_json::to_value(schemars::schema_for!(config::Config))?),
        Command::Worker { request, response } => {
            backend::worker(request, response)?;
            return Ok(serde_json::json!({"schema_version": 1, "status": "complete"}));
        }
        Command::Init(args) => {
            return project::init(args).context(report::stage("configuration", None));
        }
        _ => {}
    }
    let selectors = cli.command.selectors().context("missing selectors")?;
    let dev = matches!(cli.command, Command::Dev { .. });
    let plan = plan::resolve(selectors, dev).context(report::stage("configuration", None))?;
    match &cli.command {
        Command::Inspect(_) => Ok(serde_json::to_value(plan.inspection())?),
        Command::Doctor { build_record, .. } => {
            let record = build_record
                .as_ref()
                .map(|p| build::select_record(&plan, Some(p)))
                .transpose()?;
            let diagnosis =
                doctor::diagnose(&plan, record.as_ref().map(|r| r.snapshot_path.as_path()));
            Ok(serde_json::to_value(diagnosis)?)
        }
        Command::Build {
            no_bundle,
            execute_signing,
            ..
        } => {
            if !no_bundle {
                backend::validate(&plan, *execute_signing)
                    .context(report::stage("prerequisites", Some(&plan)))?;
            }
            let record = build::compile(&plan).context(report::stage("build", Some(&plan)))?;
            if *no_bundle {
                Ok(serde_json::to_value(record)?)
            } else {
                let plan = plan::resolve(selectors, false)
                    .context(report::stage("configuration", Some(&plan)))?;
                Ok(serde_json::to_value(
                    staging::bundle(&plan, &record, *execute_signing)
                        .context(report::stage("packaging", Some(&plan)))?,
                )?)
            }
        }
        Command::Bundle {
            build_record,
            execute_signing,
            ..
        } => {
            let record = build::select_record(&plan, build_record.as_deref())
                .context(report::stage("verification", Some(&plan)))?;
            backend::validate(&plan, *execute_signing)
                .context(report::stage("prerequisites", Some(&plan)))?;
            Ok(serde_json::to_value(
                staging::bundle(&plan, &record, *execute_signing)
                    .context(report::stage("packaging", Some(&plan)))?,
            )?)
        }
        Command::PackageTrial {
            build_record,
            execute_signing,
            ..
        } => {
            backend::validate(&plan, *execute_signing)
                .context(report::stage("prerequisites", Some(&plan)))?;
            let record = build::select_record(&plan, Some(build_record))
                .context(report::stage("verification", Some(&plan)))?;
            Ok(serde_json::to_value(
                staging::trial(&plan, &record, *execute_signing)
                    .context(report::stage("packaging", Some(&plan)))?,
            )?)
        }
        Command::Dev {
            args,
            watch,
            watch_path,
            watch_ignore,
            ..
        } => dev::run(&plan, args, *watch, watch_path, watch_ignore),
        _ => unreachable!(),
    }
}

fn main() {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "guic") {
        args.remove(1);
    }
    let json_requested = args
        .iter()
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) if json_requested && error.use_stderr() => {
            let _ = report::print(&serde_json::json!({
                "schema_version": 1, "status": "failed", "context": report::stage("configuration", None),
                "error": error.to_string()
            }));
            std::process::exit(2);
        }
        Err(error) => error.exit(),
    };
    if let Err(error) = process::install_handler()
        .and_then(|()| run(&cli))
        .and_then(|value| {
            process::check_cancelled()?;
            report::print(&value)?;
            if value.get("ready").and_then(serde_json::Value::as_bool) == Some(false) {
                std::process::exit(1);
            }
            Ok(())
        })
    {
        if cli.json {
            let _ = report::print(&serde_json::json!({
                "schema_version": 1, "status": "failed", "context": error.downcast_ref::<report::ErrorContext>(), "error": format!("{error:#}")
            }));
        } else {
            eprintln!("cargo-guic: {error:#}");
        }
        std::process::exit(1);
    }
}
