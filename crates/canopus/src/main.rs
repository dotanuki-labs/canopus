// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod cli;
mod core;
mod features;

fn main() -> anyhow::Result<()> {
    better_panic::install();
    human_panic::setup_panic!();

    let feature = cli::parse_arguments()?;
    features::execute(feature)?;
    Ok(())
}
