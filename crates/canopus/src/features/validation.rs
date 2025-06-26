// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::{CodeOwners, CodeOwnersEntry, DanglingGlobPattern, ValidationError};
use anyhow::bail;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub fn validate_codeowners(project_location: &PathBuf) -> anyhow::Result<()> {
    let codeowners_file = check_conventional_codeowners_location(project_location)?;
    log::info!("Codeowners file found at : {}", codeowners_file.to_string_lossy());

    let codeowners_content = std::fs::read_to_string(codeowners_file.as_path())?;
    let codeowners = CodeOwners::try_from(codeowners_content.as_str())?;
    log::info!("Successfully validated syntax");

    check_non_matching_glob_patterns(codeowners_file.as_path(), &codeowners)?;

    log::info!("Successfully validated path patterns");
    Ok(())
}

fn check_non_matching_glob_patterns(project_path: &Path, code_owners: &CodeOwners) -> anyhow::Result<()> {
    let glob_matchers = code_owners
        .entries
        .iter()
        .filter_map(|entry| match entry {
            CodeOwnersEntry::Rule(rule) => Some((rule.line_number, &rule.glob)),
            _ => None,
        })
        .map(|(line, glob)| (line, glob.compile_matcher()))
        .collect::<Vec<_>>();

    let walker = WalkBuilder::new(project_path).build();

    let all_paths = walker
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();

    let dangling_globs = glob_matchers
        .iter()
        .filter(|(_, glob_matcher)| all_paths.iter().any(|path| !glob_matcher.is_match(path)))
        .map(|(line, glob_matcher)| DanglingGlobPattern {
            line_number: *line,
            pattern: glob_matcher.glob().glob().to_string(),
        })
        .collect::<Vec<_>>();

    if !dangling_globs.is_empty() {
        log::info!("Found patterns that won't match any existing project files");
        bail!(ValidationError::DanglingGlobPatterns {
            patterns: dangling_globs
        })
    }

    Ok(())
}

fn check_conventional_codeowners_location(project_location: &PathBuf) -> anyhow::Result<PathBuf> {
    log::info!("Project location : {project_location:?}");

    let possible_locations = [
        project_location.join(".github/CODEOWNERS"),
        project_location.join("CODEOWNERS"),
        project_location.join("docs/CODEOWNERS"),
    ];

    let config_files = possible_locations
        .iter()
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    if config_files.is_empty() {
        bail!("no CODEOWNERS definition found in the project");
    }

    if config_files.len() > 1 {
        bail!("found multiple definitions for CODEOWNERS");
    }

    let codeowners = config_files
        .first()
        .unwrap_or_else(|| panic!("FATAL: found the CODEOWNERS file cannot construct a path to it"));

    Ok(codeowners.to_path_buf())
}
