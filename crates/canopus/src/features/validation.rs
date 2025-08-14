// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{CodeownersValidationError, DiagnosticKind, ValidationDiagnostic};
use crate::core::models::{CodeOwners, CodeOwnersEntry, CodeOwnersFile};
use crate::features::filesystem::PathWalker;
use anyhow::bail;
use itertools::Itertools;
use std::collections::HashSet;
use std::path::PathBuf;

pub fn validate_codeowners(codeowners_file: CodeOwnersFile, path_walker: impl PathWalker) -> anyhow::Result<()> {
    let codeowners = CodeOwners::try_from(codeowners_file.contents.as_str())?;
    log::info!("Successfully validated syntax");

    let validations = vec![
        check_non_matching_glob_patterns(&codeowners, &path_walker.walk()),
        check_duplicated_owners(&codeowners),
    ];

    if validations.iter().all(|check| check.is_ok()) {
        return Ok(());
    }

    let diagnostics = validations
        .into_iter()
        .filter_map(|check| check.err())
        .flat_map(|error| error.downcast::<CodeownersValidationError>())
        .flat_map(|raised| raised.diagnostics)
        .collect_vec();

    bail!(CodeownersValidationError::with(diagnostics))
}

fn check_duplicated_owners(code_owners: &CodeOwners) -> anyhow::Result<()> {
    let ownerships = &code_owners
        .entries
        .iter()
        .filter_map(|entry| match entry {
            CodeOwnersEntry::Rule(ownership) => Some(ownership),
            _ => None,
        })
        .sorted_by_key(|rule| rule.glob.glob())
        .collect_vec();

    let mut grouped_per_glob = Vec::new();

    for (glob, grouped) in &ownerships.iter().chunk_by(|rule| rule.glob.glob()) {
        let lines = grouped.into_iter().map(|rule| rule.line_number).collect_vec();

        if lines.len() > 1 {
            grouped_per_glob.push((glob.to_string(), lines));
        }
    }

    if !grouped_per_glob.is_empty() {
        let diagnostics = grouped_per_glob
            .iter()
            .map(|(glob, lines)| {
                ValidationDiagnostic::builder()
                    .kind(DiagnosticKind::DuplicateOwnership)
                    .line_number(lines[0])
                    .message(format!("{} defined multiple times : lines {:?}", glob, lines))
                    .build()
            })
            .collect_vec();

        log::info!("Found some duplicated ownership rules");

        bail!(CodeownersValidationError::with(diagnostics))
    }

    log::info!("Duplicated code owners : not found");
    Ok(())
}

fn check_non_matching_glob_patterns(code_owners: &CodeOwners, paths: &[PathBuf]) -> anyhow::Result<()> {
    let lines_and_glob_matchers = code_owners
        .entries
        .iter()
        .filter_map(|entry| match entry {
            CodeOwnersEntry::Rule(rule) => Some((rule.line_number, rule.glob.compile_matcher())),
            _ => None,
        })
        .collect_vec();

    let matching_globs = lines_and_glob_matchers
        .iter()
        .filter_map(|(_, glob_matcher)| {
            if paths.iter().any(|path| glob_matcher.is_match(path)) {
                Some(glob_matcher.glob().clone())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let diagnostics = lines_and_glob_matchers
        .iter()
        .filter(|(_, glob_matcher)| !matching_globs.contains(glob_matcher.glob()))
        .map(|(line, glob_matcher)| {
            ValidationDiagnostic::builder()
                .kind(DiagnosticKind::DanglingGlobPattern)
                .line_number(*line)
                .message(format!("{} does not match any project path", glob_matcher.glob()))
                .build()
        })
        .collect_vec();

    if !diagnostics.is_empty() {
        log::info!("Found patterns that won't match any existing project files");
        bail!(CodeownersValidationError::with(diagnostics))
    }

    log::info!("Dangling glob patterns : not found");
    Ok(())
}
