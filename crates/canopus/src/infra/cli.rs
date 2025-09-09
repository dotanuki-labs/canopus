// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::canopus::CanopusCommand;
use crate::canopus::CanopusCommand::ValidateCodeowners;
use crate::infra::cli::Commands::Validate;
use clap::{Parser, Subcommand, arg};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ValidateArguments {
    #[arg(short, long, help = "Path pointing to project root")]
    pub path: PathBuf,
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
    /// Explains lint or formatting criteria
    Validate(ValidateArguments),
}

pub fn parse_arguments() -> anyhow::Result<CanopusCommand> {
    let cli = CliParser::parse();

    let execution = match cli.command {
        Validate(args) => ValidateCodeowners(args.path),
    };

    Ok(execution)
}
