// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::canopus::Canopus;
use crate::canopus::validation::CodeOwnersValidator;
use crate::infra::github::GithubConsistencyChecker;
use crate::infra::{cli, paths};
use octorust::Client;
use octorust::auth::Credentials;

mod canopus;
mod core;
mod infra;

fn create_canopus() -> anyhow::Result<Canopus> {
    let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let credentials = std::env::var("GITHUB_TOKEN").map(Credentials::Token).ok();

    let consistency_checker = GithubConsistencyChecker::ApiBased(Client::new(user_agent, credentials)?);
    let path_walker = paths::PathWalker::GitAware;
    let codeowners_validator = CodeOwnersValidator::new(consistency_checker, path_walker);
    let canopus = Canopus::new(codeowners_validator);
    Ok(canopus)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    better_panic::install();
    human_panic::setup_panic!();
    env_logger::builder()
        .format_timestamp(None)
        .format_module_path(false)
        .format_level(false)
        .format_file(false)
        .format_target(false)
        .init();

    println!();
    let command = cli::parse_arguments()?;
    let canopus = create_canopus()?;
    canopus.execute(command).await
}
