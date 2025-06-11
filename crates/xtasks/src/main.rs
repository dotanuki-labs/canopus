// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod artifacts;
mod security;
mod sources;
mod tests;
mod utils;

use clap::{Args, Parser, Subcommand, ValueEnum};
use xshell::Shell;

#[derive(Parser)]
#[command(about, long_about = None)]
struct CliParser {
    #[command(subcommand)]
    pub task: Task,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum ArtifactType {
    Binaries,
    Extras,
}

#[derive(Args)]
struct SubjectToBuild {
    #[arg(value_enum)]
    pub what: ArtifactType,
}

#[derive(Subcommand)]
enum Task {
    /// Inspects sources for formatting and smells
    Sources,
    /// Runs unit and integration tests
    Tests,
    /// Builds project artifacts (binaries or metadata)
    Artifacts(SubjectToBuild),
    /// Detects issues with project dependencies
    Security,
}

fn main() -> anyhow::Result<()> {
    let cli = CliParser::parse();
    let shell = Shell::new()?;
    match &cli.task {
        Task::Sources => sources::check_source_files(&shell),
        Task::Tests => tests::execute_tests(&shell),
        Task::Artifacts(subject) => artifacts::assemble_artifacts(&shell, &subject.what),
        Task::Security => security::check_dependencies(&shell),
    }
}
