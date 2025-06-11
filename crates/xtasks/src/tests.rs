// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::utils::{BuildEnvironment::CI, BuildEnvironment::Local, evaluate_build_environment};
use xshell::{Shell, cmd};

pub fn execute_tests(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥 Running unit and integration tests");
    println!();

    match evaluate_build_environment() {
        CI => cmd!(shell, "cargo test").run()?,
        Local => cmd!(shell, "cargo nextest run").run()?,
    };

    Ok(())
}
