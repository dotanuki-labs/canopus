// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::ArtifactType;
use crate::utils::BuildEnvironment::{CI, Local};
use crate::utils::{docker_execution_arguments, evaluate_build_environment};
use anyhow::bail;
use sha2::{Digest, Sha256};
use std::{env, fs};
use walkdir::WalkDir;
use xshell::{Shell, cmd};

static DEFAULT_ARTIFACTS_DIR: &str = "artifacts";

pub fn assemble_artifacts(shell: &Shell, artifact_type: &ArtifactType) -> anyhow::Result<()> {
    match artifact_type {
        ArtifactType::Binaries => {
            shell.remove_path(DEFAULT_ARTIFACTS_DIR)?;
            shell.create_dir(DEFAULT_ARTIFACTS_DIR)?;
            build_targets(shell)?;
        },
        ArtifactType::Extras => extract_metadata(shell)?,
    }

    Ok(())
}

pub fn extract_metadata(shell: &Shell) -> anyhow::Result<()> {
    compute_sbom(shell)?;
    compute_checksums(shell)?;
    Ok(())
}

fn build_targets(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥 Building binaries");
    println!();

    match evaluate_build_environment() {
        CI => {
            println!("• Building on CI environment");
            let targets = evaluate_build_targets()?;

            for target in targets {
                cmd!(shell, "rustup target add {target}").run()?;
                cmd!(shell, "cargo build --release --target {target}").run()?;
                let binary = format!("target/{}/release/canopus", target);
                let destination = format!("{}/canopus-{}", DEFAULT_ARTIFACTS_DIR, target);
                shell.copy_file(&binary, &destination)?;
                cmd!(shell, "chmod +x {destination}").run()?;
            }
        },
        Local => {
            println!("• Building on local environment");
            cmd!(shell, "cargo build --release").run()?;
        },
    };
    Ok(())
}

fn compute_sbom(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥 Extracting CycloneDX SBOM from project dependencies");
    println!();

    match evaluate_build_environment() {
        CI => {
            let (volume, image) = docker_execution_arguments();
            cmd!(shell, "docker run --rm -v {volume} {image} cyclonedx").run()?;
        },
        Local => {
            cmd!(shell, "cargo cyclonedx --format json").run()?;
        },
    };

    Ok(())
}

fn compute_checksums(shell: &Shell) -> anyhow::Result<()> {
    println!();
    println!("🔥 Computing checksums for binaries");
    println!();

    let checksums = WalkDir::new(DEFAULT_ARTIFACTS_DIR)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            let path = entry.path().to_str().unwrap();
            path.contains("canopus-x86_64") || path.contains("canopus-aarch64")
        })
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let name = entry.file_name();
            let contents = fs::read(entry.path()).unwrap();
            let digest = Sha256::digest(contents);
            format!("{} : {}", name.to_string_lossy(), hex::encode(digest))
        })
        .collect::<Vec<String>>()
        .join("\n");

    let checksums_file = format!("{}/checksums.txt", DEFAULT_ARTIFACTS_DIR);
    shell.write_file(checksums_file, checksums)?;
    Ok(())
}

fn evaluate_build_targets() -> anyhow::Result<Vec<String>> {
    let runner_name = env::var("RUNNER_OS")?;
    let platform = match runner_name.as_str() {
        "Linux" => "unknown-linux-gnu",
        "macOS" => "apple-darwin",
        _ => bail!("Unsupported runner : {}", runner_name),
    };

    let archs = vec!["x86_64", "aarch64"];
    let targets = archs
        .into_iter()
        .map(|arch| format!("{}-{}", arch, platform))
        .collect::<Vec<_>>();

    Ok(targets)
}
