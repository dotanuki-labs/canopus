// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::canopus::CanopusCommand;
use crate::canopus::CanopusCommand::{RepairCodeowners, ValidateCodeowners};
use crate::infra::cli::Commands::Validate;
use Commands::Repair;
use clap::{Args, Parser, Subcommand, arg};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(version, about, long_about = None)]
struct ValidateArguments {
    #[arg(short, long, help = "Path pointing to project root")]
    pub path: PathBuf,
}

#[derive(Args, Debug)]
#[command(version, about, long_about = None)]
struct RepairArguments {
    #[arg(short, long, help = "Path pointing to project root")]
    pub path: PathBuf,

    #[arg(short, long, action, help = "Whether to preview repair results")]
    pub dry_run: Option<bool>,

    #[arg(short, long, action, help = "Whether to remove problematic lines when repairing")]
    pub remove_lines: Option<bool>,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = false)]
struct CliParser {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Repairs a CodeOwners file within a project
    Repair(RepairArguments),

    /// Validates a CodeOwners file within a project
    Validate(ValidateArguments),
}

pub fn parse_arguments() -> anyhow::Result<CanopusCommand> {
    let cli = CliParser::parse();

    let execution = match cli.command {
        Validate(args) => ValidateCodeowners(args.path),
        Repair(args) => RepairCodeowners {
            project_root: args.path,
            dry_run: args.dry_run.unwrap_or(false),
            remove_lines: args.remove_lines.unwrap_or(false),
        },
    };

    Ok(execution)
}
