// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::utils::{
    BuildEnvironment::CI, BuildEnvironment::Local, docker_execution_arguments, evaluate_build_environment,
};
use xshell::{Shell, cmd};

pub fn check_dependencies(shell: &Shell) -> anyhow::Result<()> {
    check_vulnerable_dependencies(shell)?;
    check_unused_dependencies(shell)?;
    Ok(())
}

fn check_vulnerable_dependencies(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥 Auditing project dependencies (cargo-deny)");
    println!();

    match evaluate_build_environment() {
        CI => {
            let (volume, image) = docker_execution_arguments();
            cmd!(shell, "docker run --rm -v {volume} {image} deny").run()?;
        },
        Local => {
            cmd!(shell, "cargo deny check").run()?;
        },
    };

    Ok(())
}

fn check_unused_dependencies(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥Evaluating unused dependencies (cargo-machete)");
    println!();

    match evaluate_build_environment() {
        CI => {
            let (volume, image) = docker_execution_arguments();
            cmd!(shell, "docker run --rm -v {volume} {image} machete").run()?;
        },
        Local => {
            cmd!(shell, "cargo machete").run()?;
        },
    };

    Ok(())
}
