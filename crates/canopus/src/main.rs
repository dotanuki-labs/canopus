// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::canopus::Canopus;
use crate::canopus::validation::CodeOwnersValidator;
use crate::infra::github::GithubConsistencyChecker;
use crate::infra::{cli, paths};
use octocrab::service::middleware::retry::RetryConfig;

mod canopus;
mod core;
mod infra;

fn create_canopus() -> anyhow::Result<Canopus> {
    let max_retries_per_request = 3;

    let github_pat = std::env::var("GITHUB_TOKEN").unwrap_or("".to_string());
    let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Configuration for the Github Client
    let github_client = octocrab::OctocrabBuilder::new()
        .personal_token(github_pat)
        .add_retry_config(RetryConfig::Simple(max_retries_per_request))
        .add_header(http::header::USER_AGENT, user_agent)
        .build()?;
    let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

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
