// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
use anyhow::bail;
use globset::Glob;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    GithubUser(String),
    GithubTeam(String),
    EmailAddress(String),
}

impl TryFrom<(usize, &str)> for Owner {
    type Error = anyhow::Error;

    fn try_from((line, value): (usize, &str)) -> anyhow::Result<Self> {
        if value.starts_with("@") {
            let normalized = value.replacen("@", "", 1);
            return if value.contains("/") {
                Ok(Owner::GithubTeam(normalized))
            } else {
                Ok(Owner::GithubUser(normalized))
            };
        };

        if email_address::EmailAddress::is_valid(value) {
            return Ok(Owner::EmailAddress(value.to_string()));
        };

        bail!(ValidationDiagnostic::new_syntax_issue(line, "cannot parse owner"));
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Ownership {
    pub line_number: usize,
    pub glob: Glob,
    pub owners: Vec<Owner>,
    pub inline_comment: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum CodeOwnersEntry {
    BlankLine,
    Comment(String),
    Rule(Ownership),
}

impl CodeOwnersEntry {
    pub(crate) fn try_new_comment(line_number: usize, comment: &str) -> Result<Self, ValidationDiagnostic> {
        if comment.is_empty() {
            return Err(ValidationDiagnostic::new_syntax_issue(
                line_number,
                "expected non-empty comment",
            ));
        };

        let sanitized = comment.replace("#", "").trim().to_string();
        Ok(CodeOwnersEntry::Comment(sanitized))
    }

    pub(crate) fn try_new_rule(
        line_number: usize,
        glob: Glob,
        owners: Vec<Owner>,
    ) -> Result<Self, ValidationDiagnostic> {
        if owners.is_empty() {
            return Err(ValidationDiagnostic::new_syntax_issue(
                line_number,
                "expected non-empty owners list",
            ));
        }

        let ownership = Ownership {
            line_number,
            glob,
            owners,
            inline_comment: None,
        };

        Ok(CodeOwnersEntry::Rule(ownership))
    }

    pub(crate) fn try_new_commented_rule(
        line_number: usize,
        glob: Glob,
        owners: Vec<Owner>,
        comment: &str,
    ) -> Result<Self, ValidationDiagnostic> {
        if comment.is_empty() {
            return Err(ValidationDiagnostic::new_syntax_issue(
                line_number,
                "expected non-empty comment",
            ));
        };

        if owners.is_empty() {
            return Err(ValidationDiagnostic::new_syntax_issue(
                line_number,
                "expected non-empty owners list",
            ));
        }

        let ownership = Ownership {
            line_number,
            glob,
            owners,
            inline_comment: Some(comment.to_string()),
        };

        Ok(CodeOwnersEntry::Rule(ownership))
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
                    let issue = ValidationDiagnostic::new_syntax_issue(line_number, "invalid glob pattern");
                    diagnostics.push(issue);
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
                    match Owner::try_from((line_number, item)) {
                        Ok(owner) => {
                            owners.push(owner);
                        },
                        Err(_) => {
                            let issue = ValidationDiagnostic::new_syntax_issue(line_number, "cannot parse owner");
                            diagnostics.push(issue)
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

#[derive(Debug, PartialEq)]
pub struct CodeOwners {
    pub entries: Vec<CodeOwnersEntry>,
}

impl TryFrom<&str> for CodeOwners {
    type Error = anyhow::Error;

    fn try_from(content: &str) -> anyhow::Result<Self> {
        let lines = content.lines();

        let mut entries: Vec<CodeOwnersEntry> = vec![];
        let mut diagnostics: Vec<ValidationDiagnostic> = vec![];

        for (line_number, line_contents) in lines.enumerate() {
            match CodeOwnersEntry::try_from((line_number, line_contents)) {
                Ok(entry) => entries.push(entry),
                Err(mut error) => diagnostics.append(&mut error.diagnostics),
            }
        }

        if !diagnostics.is_empty() {
            bail!(CodeownersValidationError { diagnostics });
        }

        Ok(CodeOwners { entries })
    }
}

pub struct CodeOwnersFile {
    pub path: PathBuf,
    pub contents: String,
}

impl CodeOwnersFile {
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
}

impl TryFrom<PathBuf> for CodeOwnersFile {
    type Error = anyhow::Error;

    fn try_from(value: PathBuf) -> anyhow::Result<Self> {
        let codeowners_file = Self::check_conventional_codeowners_location(&value)?;
        log::debug!("Codeowners config found at : {}", &codeowners_file.to_string_lossy());

        let codeowners_content = std::fs::read_to_string(codeowners_file.as_path())?;
        Ok(Self {
            path: codeowners_file,
            contents: codeowners_content,
        })
    }
}
