// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{CodeownersValidationError, DiagnosticKind, ValidationDiagnostic};
use anyhow::bail;
use globset::Glob;
use itertools::Itertools;
use lazy_regex::{Lazy, Regex};
use std::path::PathBuf;

// From https://github.com/dead-claudia/github-limits
static GITHUB_HANDLE_REGEX: &Lazy<Regex, fn() -> Regex> = lazy_regex::regex!(r#"^[a-zA-Z\d](-?[a-zA-Z\d]){0,38}$"#);
static GITHUB_TEAM_REGEX: &Lazy<Regex, fn() -> Regex> = lazy_regex::regex!(r#"^[a-zA-Z\d](-?[a-zA-Z\d]){0,254}$"#);

type ParsedLine = (usize, String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EmailHandle(String);

impl EmailHandle {
    pub fn new(email: String) -> Self {
        Self(email)
    }
}

impl From<&str> for EmailHandle {
    fn from(value: &str) -> Self {
        EmailHandle::new(value.to_string())
    }
}

impl TryFrom<ParsedLine> for EmailHandle {
    type Error = ValidationDiagnostic;

    fn try_from((line, email): ParsedLine) -> Result<Self, Self::Error> {
        if email_address::EmailAddress::is_valid(&email) {
            return Ok(Self(email));
        };

        let diagnostic = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(line)
            .description("cannot parse owner from email address")
            .build();

        Err(diagnostic)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GithubIdentityHandle(String);

impl GithubIdentityHandle {
    pub fn new(handle: String) -> Self {
        Self(handle)
    }
}

impl From<&str> for GithubIdentityHandle {
    fn from(value: &str) -> Self {
        GithubIdentityHandle::new(value.to_string())
    }
}

impl TryFrom<ParsedLine> for GithubIdentityHandle {
    type Error = ValidationDiagnostic;

    fn try_from((line, github_handle): ParsedLine) -> Result<Self, Self::Error> {
        if GITHUB_HANDLE_REGEX.is_match(&github_handle) {
            return Ok(Self(github_handle));
        };

        let diagnostic = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(line)
            .description("invalid github handle")
            .build();

        Err(diagnostic)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GithubTeamHandle {
    pub organization: GithubIdentityHandle,
    pub name: String,
}

impl GithubTeamHandle {
    pub fn new(organization: GithubIdentityHandle, name: String) -> Self {
        Self { organization, name }
    }
}

impl TryFrom<ParsedLine> for GithubTeamHandle {
    type Error = ValidationDiagnostic;

    fn try_from((line, team_handle): ParsedLine) -> Result<Self, Self::Error> {
        let parts = team_handle.split('/').collect_vec();

        if parts.len() > 2 {
            let diagnostic = ValidationDiagnostic::builder()
                .kind(DiagnosticKind::InvalidSyntax)
                .line_number(line)
                .description("cannot parse github team handle")
                .build();

            return Err(diagnostic);
        }

        let org_name = parts[0].to_owned();
        let team_name = parts[1].to_owned();

        let organization = GithubIdentityHandle::try_from((line, org_name))?;
        if GITHUB_TEAM_REGEX.is_match(&team_name) {
            let team = GithubTeamHandle::new(organization, team_name);
            return Ok(team);
        };

        let diagnostic = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(line)
            .description("invalid github team handle")
            .build();

        Err(diagnostic)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    GithubUser(GithubIdentityHandle),
    GithubTeam(GithubTeamHandle),
    EmailAddress(EmailHandle),
}

impl TryFrom<ParsedLine> for Owner {
    type Error = ValidationDiagnostic;

    fn try_from((line, value): ParsedLine) -> Result<Self, Self::Error> {
        println!("Tryfrom (owner) : line = {}, value = {}", line, value);
        match value {
            _ if value.starts_with("@") => {
                let normalized = value.trim_start_matches("@").to_owned();
                let owner = if value.contains("/") {
                    let team_handle = GithubTeamHandle::try_from((line, normalized))?;
                    Owner::GithubTeam(team_handle)
                } else {
                    let identity_handle = GithubIdentityHandle::try_from((line, normalized))?;
                    Owner::GithubUser(identity_handle)
                };

                Ok(owner)
            },
            _ if value.contains("@") => {
                let email_handle = EmailHandle::try_from((line, value))?;
                let owner = Owner::EmailAddress(email_handle);
                Ok(owner)
            },
            _ => {
                let diagnostic = ValidationDiagnostic::builder()
                    .kind(DiagnosticKind::InvalidSyntax)
                    .line_number(line)
                    .description("cannot parse owner")
                    .build();

                Err(diagnostic)
            },
        }
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
        Self::check_non_empty_comment(line_number, comment)?;

        let sanitized = comment.replace("#", "").trim().to_string();
        Ok(CodeOwnersEntry::Comment(sanitized))
    }

    pub(crate) fn try_new_rule(
        line_number: usize,
        glob: Glob,
        owners: Vec<Owner>,
    ) -> Result<Self, ValidationDiagnostic> {
        Self::check_non_empty_owners_list(line_number, &owners)?;

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
        Self::check_non_empty_comment(line_number, comment)?;
        Self::check_non_empty_owners_list(line_number, &owners)?;

        let ownership = Ownership {
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
                .kind(DiagnosticKind::InvalidSyntax)
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
                .kind(DiagnosticKind::InvalidSyntax)
                .line_number(line_number)
                .description("expected non-empty owners list")
                .build();

            return Err(empty_owners_list);
        }

        Ok(())
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
                        .kind(DiagnosticKind::InvalidSyntax)
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
                                .kind(DiagnosticKind::InvalidSyntax)
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
            bail!(CodeownersValidationError::with(diagnostics));
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
