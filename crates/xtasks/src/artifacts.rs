// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::ArtifactType;
use crate::utils::BuildEnvironment::{CI, Local};
use crate::utils::{docker_execution_arguments, evaluate_build_environment};
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
            let is_linux_gha_runner = env::var("RUNNER_OS")
                .map(|os| os.eq_ignore_ascii_case("linux"))
                .unwrap_or(false);

            let linux_build_params = [("x86_64", "unknown-linux-musl"), ("aarch64", "unknown-linux-musl")];

            let apple_build_params = [("x86_64", "apple-darwin"), ("aarch64", "apple-darwin")];

            let actual_params = if is_linux_gha_runner {
                linux_build_params
            } else {
                apple_build_params
            };

            println!("OS = {}", env::var("RUNNER_OS")?);
            println!("Build params = {:?}", actual_params);

            for (arch, target) in actual_params {
                if is_linux_gha_runner {
                    let current_dir = env::current_dir()?;
                    let volume = format!("{}:/home/rust/src", current_dir.to_str().unwrap());
                    let docker_image = format!("docker.io/blackdex/rust-musl:{arch}-musl");
                    cmd!(shell, "docker run --rm -v {volume} {docker_image} cargo build --release --target {arch}-{target} --package canopus").run()?;
                } else {
                    cmd!(shell, "rustup target add {arch}-{target}").run()?;
                    cmd!(
                        shell,
                        "cargo build --release --target {arch}-{target} --package canopus"
                    )
                    .run()?;
                }

                let binary = format!("target/{arch}-{target}/release/canopus");
                let destination = format!("{DEFAULT_ARTIFACTS_DIR}/canopus-{target}");
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

    let checksums_file = format!("{DEFAULT_ARTIFACTS_DIR}/checksums.txt");
    shell.write_file(checksums_file, checksums)?;
    Ok(())
}
