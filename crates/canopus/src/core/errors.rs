// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq)]
pub enum StructuralIssue {
    InvalidSyntax,
    DanglingGlobPattern,
    DuplicateOwnership,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConsistencyIssue {
    CannotListMembersInTheOrganization,
    CannotVerifyUser(GithubIdentityHandle),
    CannotVerifyTeam(GithubTeamHandle),
    UserDoesNotExist(GithubIdentityHandle),
    OrganizationDoesNotExist(GithubIdentityHandle),
    TeamDoesNotExistWithinOrganization(GithubTeamHandle),
    UserDoesNotBelongToOrganization(GithubIdentityHandle),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagnosticKind {
    Structural(StructuralIssue),
    Consistency(ConsistencyIssue),
}

impl Display for DiagnosticKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticKind::Structural(_) => write!(f, "structure"),
            DiagnosticKind::Consistency(_) => write!(f, "consistency"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationDiagnostic {
    kind: DiagnosticKind,
    line: usize,
    context: String,
}

#[derive(Default)]
pub struct ValidationDiagnosticBuilder {
    kind: Option<DiagnosticKind>,
    line: Option<usize>,
    context: Option<String>,
}

impl ValidationDiagnosticBuilder {
    pub fn kind(mut self, kind: DiagnosticKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn line_number(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn description(mut self, context: &str) -> Self {
        self.context = Some(context.to_string());
        self
    }

    pub fn message(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    pub fn build(self) -> ValidationDiagnostic {
        ValidationDiagnostic {
            kind: self.kind.expect("missing diagnostic kind"),
            line: self.line.expect("missing related line in codeowners file"),
            context: self.context.expect("missing context for this diagnostic"),
        }
    }
}

impl ValidationDiagnostic {
    pub fn builder() -> ValidationDiagnosticBuilder {
        ValidationDiagnosticBuilder::default()
    }
}

impl Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] L{} : {}", self.kind, self.line, self.context)
    }
}

impl std::error::Error for ValidationDiagnostic {
    // Already satisfied
}

impl From<ValidationDiagnostic> for CodeownersValidationError {
    fn from(value: ValidationDiagnostic) -> Self {
        CodeownersValidationError {
            diagnostics: vec![value],
        }
    }
}

impl From<CodeownersValidationError> for Vec<ValidationDiagnostic> {
    fn from(value: CodeownersValidationError) -> Self {
        value.diagnostics
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeownersValidationError {
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl CodeownersValidationError {
    pub fn with(diagnostics: Vec<ValidationDiagnostic>) -> Self {
        Self { diagnostics }
    }
}

impl Display for CodeownersValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let messages = self
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>();

        f.write_str(&messages.join("\n"))
    }
}

impl std::error::Error for CodeownersValidationError {
    // Already satisfied
}

impl From<anyhow::Result<()>> for CodeownersValidationError {
    fn from(value: anyhow::Result<()>) -> Self {
        value.expect_err("expecting an error").downcast().unwrap()
    }
}

#[cfg(test)]
pub mod test_helpers {
    use crate::core::errors::{ConsistencyIssue, DiagnosticKind, StructuralIssue};
    use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};

    pub struct DiagnosticKindFactory;

    impl DiagnosticKindFactory {
        pub fn invalid_syntax() -> DiagnosticKind {
            DiagnosticKind::Structural(StructuralIssue::InvalidSyntax)
        }

        pub fn dangling_glob_pattern() -> DiagnosticKind {
            DiagnosticKind::Structural(StructuralIssue::DanglingGlobPattern)
        }

        pub fn duplicate_ownership() -> DiagnosticKind {
            DiagnosticKind::Structural(StructuralIssue::DuplicateOwnership)
        }

        pub fn team_does_not_exist(organization: &str, team: &str) -> DiagnosticKind {
            let handle = GithubTeamHandle::new(GithubIdentityHandle::new(organization.to_string()), team.to_string());
            DiagnosticKind::Consistency(ConsistencyIssue::TeamDoesNotExistWithinOrganization(handle))
        }

        pub fn user_does_not_belong_to_organization(name: &str) -> DiagnosticKind {
            let handle = GithubIdentityHandle::new(name.to_string());
            DiagnosticKind::Consistency(ConsistencyIssue::UserDoesNotBelongToOrganization(handle))
        }
    }
}
