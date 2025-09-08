// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::canopus::Canopus;
use crate::canopus::validation::CodeOwnersValidator;
use crate::infra::github::GithubConsistencyChecker;
use crate::infra::{cli, paths};
use octorust::Client;
use octorust::auth::Credentials;
use policies::ExponentialBackoff;
use reqwest_retry::policies;
use tikv_jemallocator::Jemalloc;

mod canopus;
mod core;
mod infra;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn create_canopus() -> anyhow::Result<Canopus> {
    // Configuration for the underlying HTTP client
    let max_retries_per_request = 3;
    let base_http_client = reqwest::Client::builder().build()?;

    let exponential_backoff = ExponentialBackoff::builder().build_with_max_retries(max_retries_per_request);
    let retry_middleware = reqwest_retry::RetryTransientMiddleware::new_with_policy(exponential_backoff);
    let custom_http_client = reqwest_middleware::ClientBuilder::new(base_http_client)
        .with(retry_middleware)
        .build();

    // Configuration for the Github Client
    let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    let credentials = std::env::var("GITHUB_TOKEN").map(Credentials::Token).ok();
    let github_client = Client::custom(user_agent, credentials, custom_http_client);
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
