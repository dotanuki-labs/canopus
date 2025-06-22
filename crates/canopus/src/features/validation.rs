// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::CodeOwners;
use std::path::PathBuf;

pub fn validate_codeowners(project_location: &PathBuf) -> anyhow::Result<()> {
    let codeowners_file = check_conventional_codeowners_location(project_location)?;
    log::info!("Codeowners file found at : {}", codeowners_file.to_string_lossy());

    let codeowners_content = std::fs::read_to_string(codeowners_file)?;
    let _ = CodeOwners::try_from(codeowners_content.as_str())?;
    log::info!("Successfully validated syntax");
    Ok(())
}

pub fn check_conventional_codeowners_location(project_location: &PathBuf) -> anyhow::Result<PathBuf> {
    println!("Project location : {:?}", project_location);

    let possible_locations = [
        project_location.join(".github/CODEOWNERS"),
        project_location.join("CODEOWNERS"),
        project_location.join("docs/CODEOWNERS"),
    ];

    let config_files = possible_locations
        .iter()
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    println!("Codeowners files : {:?}", config_files);

    if config_files.is_empty() {
        anyhow::bail!("No CODEOWNERS definition found in the project");
    }

    if config_files.len() > 1 {
        anyhow::bail!("Found multiple definitions for CODEOWNERS");
    }

    let codeowners = config_files
        .first()
        .unwrap_or_else(|| panic!("FATAL: found the CODEOWNERS file cannot construct a path to it"));

    Ok(codeowners.to_path_buf())
}
