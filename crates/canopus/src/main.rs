// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::infra::cli;

mod core;
mod features;
mod infra;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    better_panic::install();
    human_panic::setup_panic!();

    let feature = cli::parse_arguments()?;
    features::execute(feature).await
}
