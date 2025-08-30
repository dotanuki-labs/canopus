// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{CodeownersValidationError, DiagnosticKind, StructuralIssue, ValidationDiagnostic};
use crate::core::models::handles::Owner;
use anyhow::bail;
use globset::Glob;
use itertools::Itertools;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OwnershipRule {
    pub line_number: usize,
    pub glob: Glob,
    pub owners: Vec<Owner>,
    pub inline_comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CodeOwnersEntry {
    BlankLine,
    Comment(String),
    Rule(OwnershipRule),
}

impl CodeOwnersEntry {
    fn try_new_comment(line_number: usize, comment: &str) -> Result<Self, ValidationDiagnostic> {
        Self::check_non_empty_comment(line_number, comment)?;

        let sanitized = comment.replace("#", "").trim().to_string();
        Ok(CodeOwnersEntry::Comment(sanitized))
    }

    fn try_new_rule(line_number: usize, glob: Glob, owners: Vec<Owner>) -> Result<Self, ValidationDiagnostic> {
        Self::check_non_empty_owners_list(line_number, &owners)?;

        let ownership = OwnershipRule {
            line_number,
            glob,
            owners,
            inline_comment: None,
        };

        Ok(CodeOwnersEntry::Rule(ownership))
    }

    fn try_new_commented_rule(
        line_number: usize,
        glob: Glob,
        owners: Vec<Owner>,
        comment: &str,
    ) -> Result<Self, ValidationDiagnostic> {
        Self::check_non_empty_comment(line_number, comment)?;
        Self::check_non_empty_owners_list(line_number, &owners)?;

        let ownership = OwnershipRule {
            line_number,
            glob,
            owners,
            inline_comment: Some(comment.to_string()),
        };

        Ok(CodeOwnersEntry::Rule(ownership))
    }

    fn check_non_empty_comment(line_number: usize, comment: &str) -> Result<(), ValidationDiagnostic> {
        if comment.is_empty() {
            let empty_comment = ValidationDiagnostic::builder()
                .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
                .line_number(line_number)
                .description("expected non-empty comment")
                .build();

            return Err(empty_comment);
        };

        Ok(())
    }

    fn check_non_empty_owners_list(line_number: usize, owners: &[Owner]) -> Result<(), ValidationDiagnostic> {
        if owners.is_empty() {
            let empty_owners_list = ValidationDiagnostic::builder()
                .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
                .line_number(line_number)
                .description("expected non-empty owners list")
                .build();

            return Err(empty_owners_list);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn detailed_ownership(line_number: usize, glob: &str, owner: &str, comment: &str) -> CodeOwnersEntry {
        CodeOwnersEntry::try_from((line_number, format!("{glob} {owner} {comment}").as_str())).unwrap()
    }

    #[cfg(test)]
    pub fn ownership(line_number: usize, glob: &str, owner: &str) -> CodeOwnersEntry {
        CodeOwnersEntry::try_from((line_number, format!("{glob} {owner}").as_str())).unwrap()
    }

    #[cfg(test)]
    pub fn comment(line_number: usize, comment: &str) -> CodeOwnersEntry {
        CodeOwnersEntry::try_from((line_number, comment)).unwrap()
    }
}

impl TryFrom<(usize, &str)> for CodeOwnersEntry {
    type Error = CodeownersValidationError;

    fn try_from((line_number, line_contents): (usize, &str)) -> Result<Self, CodeownersValidationError> {
        if line_contents.is_empty() {
            Ok(CodeOwnersEntry::BlankLine)
        } else if line_contents.starts_with("#") {
            CodeOwnersEntry::try_new_comment(line_number, line_contents).map_err(|e| e.into())
        } else {
            let mut parts = line_contents.split_whitespace();

            let Some(raw_pattern) = parts.next() else {
                panic!("L{line_number} : expecting non-empty line")
            };

            let mut diagnostics: Vec<ValidationDiagnostic> = vec![];

            let glob_pattern = match Glob::new(raw_pattern) {
                Ok(glob) => Some(glob),
                Err(_) => {
                    let invalid_glob = ValidationDiagnostic::builder()
                        .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
                        .line_number(line_number)
                        .description("invalid glob pattern")
                        .build();

                    diagnostics.push(invalid_glob);
                    None
                },
            };

            let mut owners: Vec<Owner> = vec![];
            let mut inline_comments: Vec<&str> = vec![];
            let mut inline_comment_detected = false;

            for item in parts {
                if item == "#" {
                    inline_comment_detected = true;
                    continue;
                }

                if inline_comment_detected {
                    inline_comments.push(item);
                } else {
                    match Owner::try_from((line_number, item.to_string())) {
                        Ok(owner) => {
                            owners.push(owner);
                        },
                        Err(_) => {
                            let invalid_owner = ValidationDiagnostic::builder()
                                .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
                                .line_number(line_number)
                                .description("cannot parse owner")
                                .build();

                            diagnostics.push(invalid_owner)
                        },
                    }
                }
            }

            if !diagnostics.is_empty() || glob_pattern.is_none() {
                return Err(CodeownersValidationError { diagnostics });
            }

            let glob = glob_pattern.unwrap();

            if inline_comment_detected {
                let inline_comment = inline_comments.join(" ");
                CodeOwnersEntry::try_new_commented_rule(line_number, glob, owners, &inline_comment)
                    .map_err(|e| e.into())
            } else {
                CodeOwnersEntry::try_new_rule(line_number, glob, owners).map_err(|e| e.into())
            }
        }
    }
}

#[derive(Debug)]
pub struct CodeOwnersContext {
    pub project_root: PathBuf,
    pub location: PathBuf,
    pub contents: String,
}

impl CodeOwnersContext {
    fn check_conventional_codeowners_location(project_location: &Path) -> anyhow::Result<PathBuf> {
        log::info!("Project location : {}", project_location.to_string_lossy());

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
            bail!("found multiple CODEOWNERS definitions");
        }

        let codeowners = config_files
            .first()
            .unwrap_or_else(|| panic!("FATAL: found the CODEOWNERS file cannot construct a path to it"));

        Ok(codeowners.to_path_buf())
    }
}

impl TryFrom<PathBuf> for CodeOwnersContext {
    type Error = anyhow::Error;

    fn try_from(value: PathBuf) -> anyhow::Result<Self> {
        let codeowners_file = Self::check_conventional_codeowners_location(value.as_path())?;

        let codeowners_content = std::fs::read_to_string(codeowners_file.as_path())?;
        let attributes = Self {
            project_root: value,
            location: codeowners_file,
            contents: codeowners_content,
        };

        log::info!(
            "Codeowners config found at : {}",
            &attributes.location.to_string_lossy()
        );

        Ok(attributes)
    }
}

#[derive(Debug, PartialEq)]
pub struct OwnershipRecord {
    pub line_number: usize,
    pub glob: Glob,
}

impl OwnershipRecord {
    pub fn new(line_number: usize, glob: Glob) -> Self {
        Self { line_number, glob }
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeOwners {
    pub entries: Vec<CodeOwnersEntry>,
    ownerships: HashMap<Owner, Vec<OwnershipRecord>>,
}

impl CodeOwners {
    pub fn new(entries: Vec<CodeOwnersEntry>, ownerships: HashMap<Owner, Vec<OwnershipRecord>>) -> Self {
        Self { entries, ownerships }
    }

    pub fn unique_owners(&self) -> Vec<&Owner> {
        self.ownerships.keys().collect_vec()
    }

    pub fn occurrences(&self, owner: &Owner) -> Vec<usize> {
        match &self.ownerships.get(owner) {
            None => vec![],
            Some(records) => records.iter().map(|record| record.line_number).collect(),
        }
    }
}

impl TryFrom<&str> for CodeOwners {
    type Error = anyhow::Error;

    fn try_from(content: &str) -> anyhow::Result<Self> {
        let lines = content.lines();

        let mut entries: Vec<CodeOwnersEntry> = vec![];
        let mut ownerships: HashMap<Owner, Vec<OwnershipRecord>> = HashMap::new();
        let mut diagnostics: Vec<ValidationDiagnostic> = vec![];

        for (line_number, line_contents) in lines.enumerate() {
            match CodeOwnersEntry::try_from((line_number, line_contents)) {
                Ok(entry) => {
                    entries.push(entry.clone());

                    if let CodeOwnersEntry::Rule(rule) = entry {
                        for owner in rule.owners {
                            if !ownerships.contains_key(&owner) {
                                ownerships.insert(owner.clone(), vec![]);
                            }

                            let new_record = OwnershipRecord::new(line_number, rule.glob.clone());
                            let records = ownerships.get_mut(&owner).unwrap();
                            records.push(new_record);
                        }
                    }
                },
                Err(mut error) => diagnostics.append(&mut error.diagnostics),
            }
        }

        if !diagnostics.is_empty() {
            bail!(CodeownersValidationError::with(diagnostics));
        }

        Ok(CodeOwners::new(entries, ownerships))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::models::codeowners::CodeOwnersContext;
    use assertor::StringAssertion;
    use indoc::indoc;
    use std::fs;
    use temp_dir::TempDir;

    #[test]
    fn should_report_codeowners_not_found() {
        let temp_dir = TempDir::new().expect("Cant create temp dir");

        let project_path = temp_dir.path().to_path_buf();

        let context = CodeOwnersContext::try_from(project_path);

        assertor::assert_that!(context.unwrap_err().to_string()).contains("no CODEOWNERS definition found");
    }

    #[test]
    fn should_detect_multiple_codeowners() {
        let codeowners = indoc! {"
            # Basic syntax
            *.rs    @dotanuki/crabbers
        "};

        let temp_dir = TempDir::new().expect("Cant create temp dir");

        let some_config = temp_dir.path().join("CODEOWNERS");
        fs::write(&some_config, codeowners).expect("failed to write content to CODEOWNERS file");

        fs::create_dir_all(temp_dir.child(".github")).expect("Failed to create .github dir");

        let another_config = temp_dir.path().join(".github/CODEOWNERS");
        fs::write(&another_config, codeowners).expect("failed to write content to CODEOWNERS file");

        let project_path = some_config.parent().unwrap().to_path_buf();

        let context = CodeOwnersContext::try_from(project_path);

        assertor::assert_that!(context.unwrap_err().to_string()).contains("multiple CODEOWNERS definitions");
    }
}
