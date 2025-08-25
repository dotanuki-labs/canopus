// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{DiagnosticKind, StructuralIssue, ValidationDiagnostic};
use crate::core::models::ParsedLine;
use itertools::Itertools;
use lazy_regex::{Lazy, Regex};

// From https://github.com/dead-claudia/github-limits
static GITHUB_HANDLE_REGEX: &Lazy<Regex, fn() -> Regex> = lazy_regex::regex!(r#"^[a-zA-Z\d](-?[a-zA-Z\d]){0,38}$"#);
static GITHUB_TEAM_REGEX: &Lazy<Regex, fn() -> Regex> = lazy_regex::regex!(r#"^[a-zA-Z\d](-?[a-zA-Z\d]){0,254}$"#);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EmailHandle(String);

impl TryFrom<ParsedLine> for EmailHandle {
    type Error = ValidationDiagnostic;

    fn try_from((line, email): ParsedLine) -> Result<Self, Self::Error> {
        if email_address::EmailAddress::is_valid(&email) {
            return Ok(Self(email));
        };

        let diagnostic = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
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

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl TryFrom<ParsedLine> for GithubIdentityHandle {
    type Error = ValidationDiagnostic;

    fn try_from((line, github_handle): ParsedLine) -> Result<Self, Self::Error> {
        if GITHUB_HANDLE_REGEX.is_match(&github_handle) {
            return Ok(Self(github_handle));
        };

        let diagnostic = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
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
                .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
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
            .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
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
                    .kind(DiagnosticKind::Structural(StructuralIssue::InvalidSyntax))
                    .line_number(line)
                    .description("cannot parse owner")
                    .build();

                Err(diagnostic)
            },
        }
    }
}

#[cfg(test)]
impl From<&str> for Owner {
    fn from(value: &str) -> Self {
        Owner::try_from((0, value.to_string())).unwrap()
    }
}
