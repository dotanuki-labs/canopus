// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
use crate::core::models::{CodeOwners, CodeOwnersEntry, CodeOwnersFile};
use crate::features::filesystem::PathWalker;
use anyhow::bail;
use std::path::PathBuf;

pub fn validate_codeowners(codeowners_file: CodeOwnersFile, path_walker: impl PathWalker) -> anyhow::Result<()> {
    let codeowners = CodeOwners::try_from(codeowners_file.contents.as_str())?;
    log::info!("Successfully validated syntax");

    let paths = path_walker.walk();
    check_non_matching_glob_patterns(&codeowners, &paths)?;
    log::info!("Successfully validated path patterns");
    Ok(())
}

fn check_non_matching_glob_patterns(code_owners: &CodeOwners, paths: &[PathBuf]) -> anyhow::Result<()> {
    let glob_matchers = code_owners
        .entries
        .iter()
        .filter_map(|entry| match entry {
            CodeOwnersEntry::Rule(rule) => Some((rule.line_number, &rule.glob)),
            _ => None,
        })
        .map(|(line, glob)| (line, glob.compile_matcher()))
        .collect::<Vec<_>>();

    let dangling_globs = glob_matchers
        .iter()
        .filter(|(_, glob_matcher)| paths.iter().any(|path| !glob_matcher.is_match(path)))
        .map(|(line, glob_matcher)| {
            ValidationDiagnostic::new_dangling_glob_issue(
                *line,
                format!("{} does not match any project path", glob_matcher.glob().glob()).as_str(),
            )
        })
        .collect::<Vec<_>>();

    if !dangling_globs.is_empty() {
        log::info!("Found patterns that won't match any existing project files");
        bail!(CodeownersValidationError {
            diagnostics: dangling_globs
        });
    }

    Ok(())
}
