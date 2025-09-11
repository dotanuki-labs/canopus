// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::models::codeowners::CodeOwners;
use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle, Owner};
use std::fmt::{Display, Formatter};

pub mod codeowners;
pub mod config;
pub mod handles;

#[derive(Clone, Debug, PartialEq)]
pub enum ValidationOutcome {
    NoIssues,
    IssuesDetected(Vec<ValidationIssue>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StructuralIssue {
    InvalidSyntax,
    DanglingGlobPattern,
    DuplicateOwnership,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConsistencyIssue {
    CannotListMembersInTheOrganization(String),
    CannotVerifyUser(GithubIdentityHandle),
    CannotVerifyTeam(GithubTeamHandle),
    OrganizationDoesNotExist(GithubIdentityHandle),
    OutsiderUser(GithubIdentityHandle),
    TeamDoesNotMatchOrganization(GithubTeamHandle),
    TeamDoesNotExist(GithubTeamHandle),
    UserDoesNotExist(GithubIdentityHandle),
}

impl ConsistencyIssue {
    // Pragmatic way to convert a consistency issue to a validation one,
    // which requires aggregate contextual information from CodeOwners
    pub fn to_validation_issue(&self, code_owners: &CodeOwners) -> ValidationIssue {
        // We will build a triple for each variant of ConsistencyIssue
        let (issue, occurrence, reason) = match self {
            ConsistencyIssue::UserDoesNotExist(handle) => {
                let owner = Owner::GithubUser(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!("'{}' user does not exist", handle.inner()),
                )
            },
            ConsistencyIssue::OrganizationDoesNotExist(handle) => {
                let owner = Owner::GithubUser(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!("'{}' organization does not exist", handle.inner()),
                )
            },
            ConsistencyIssue::TeamDoesNotExist(handle) => {
                let owner = Owner::GithubTeam(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!(
                        "'{}' team does not belong to '{}' organization",
                        handle.name,
                        handle.organization.inner()
                    ),
                )
            },
            ConsistencyIssue::OutsiderUser(handle) => {
                let owner = Owner::GithubUser(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!("'{}' user does not belong to this organization", handle.inner()),
                )
            },
            ConsistencyIssue::CannotVerifyUser(handle) => {
                let owner = Owner::GithubUser(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!("cannot confirm if user '{}' exists", handle.inner()),
                )
            },
            ConsistencyIssue::CannotVerifyTeam(handle) => {
                let owner = Owner::GithubTeam(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!(
                        "cannot confirm whether '{}/{}' team exists",
                        handle.organization.inner(),
                        handle.name
                    ),
                )
            },
            ConsistencyIssue::CannotListMembersInTheOrganization(organization) => (
                self,
                usize::MAX, // Super hacky solution since we can't assign a line in this case
                format!("failed to list members that belong to '{}' organization", organization),
            ),
            ConsistencyIssue::TeamDoesNotMatchOrganization(handle) => {
                let owner = Owner::GithubTeam(handle.clone());
                let first_occurrence = code_owners.occurrences(&owner)[0];
                (
                    self,
                    first_occurrence,
                    format!(
                        "team '{}/{}' does not belong to this organization",
                        handle.organization.inner(),
                        handle.name
                    ),
                )
            },
        };

        // We use the triple to populate the builder
        ValidationIssue::builder()
            .kind(IssueKind::Consistency(issue.clone()))
            .line_number(occurrence)
            .message(reason)
            .build()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfigurationIssue {
    EmailOwnerForbidden,
    OnlyGithubTeamOwnerAllowed,
    OnlyOneOwnerPerEntry,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IssueKind {
    Structural(StructuralIssue),
    Consistency(ConsistencyIssue),
    Configuration(ConfigurationIssue),
}

impl Display for IssueKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueKind::Structural(_) => write!(f, "structure"),
            IssueKind::Consistency(_) => write!(f, "consistency"),
            IssueKind::Configuration(_) => write!(f, "configuration"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationIssue {
    pub line: usize,
    pub context: String,
    kind: IssueKind,
}

#[derive(Default)]
pub struct ValidationIssueBuilder {
    kind: Option<IssueKind>,
    line: Option<usize>,
    context: Option<String>,
}

impl ValidationIssueBuilder {
    pub fn kind(mut self, kind: IssueKind) -> Self {
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

    pub fn build(self) -> ValidationIssue {
        ValidationIssue {
            kind: self.kind.expect("missing diagnostic kind"),
            line: self.line.expect("missing related line in codeowners file"),
            context: self.context.expect("missing context for this diagnostic"),
        }
    }
}

impl ValidationIssue {
    pub fn builder() -> ValidationIssueBuilder {
        ValidationIssueBuilder::default()
    }
}

impl Display for ValidationIssue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.line == usize::MAX {
            write!(f, "Preconditions : {} [{}]", self.context, self.kind)
        } else {
            write!(f, "L{} : {} [{}]", self.line + 1, self.context, self.kind)
        }
    }
}

impl From<ValidationIssue> for CodeownersParsingOutcome {
    fn from(value: ValidationIssue) -> Self {
        CodeownersParsingOutcome(vec![value])
    }
}

impl From<CodeownersParsingOutcome> for Vec<ValidationIssue> {
    fn from(value: CodeownersParsingOutcome) -> Self {
        value.0
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeownersParsingOutcome(pub Vec<ValidationIssue>);

#[cfg(test)]
pub mod test_helpers {
    use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
    use crate::core::models::{ConfigurationIssue, ConsistencyIssue, IssueKind, StructuralIssue};

    pub struct ValidationIssueKindFactory;

    impl ValidationIssueKindFactory {
        pub fn invalid_syntax() -> IssueKind {
            IssueKind::Structural(StructuralIssue::InvalidSyntax)
        }

        pub fn dangling_glob_pattern() -> IssueKind {
            IssueKind::Structural(StructuralIssue::DanglingGlobPattern)
        }

        pub fn duplicate_ownership() -> IssueKind {
            IssueKind::Structural(StructuralIssue::DuplicateOwnership)
        }

        pub fn team_does_not_exist(organization: &str, team: &str) -> IssueKind {
            let handle = GithubTeamHandle::new(GithubIdentityHandle::new(organization.to_string()), team.to_string());
            IssueKind::Consistency(ConsistencyIssue::TeamDoesNotExist(handle))
        }

        pub fn user_does_not_belong_to_organization(name: &str) -> IssueKind {
            let handle = GithubIdentityHandle::new(name.to_string());
            IssueKind::Consistency(ConsistencyIssue::OutsiderUser(handle))
        }

        pub fn github_owners_only() -> IssueKind {
            IssueKind::Configuration(ConfigurationIssue::EmailOwnerForbidden)
        }

        pub fn github_team_owners_only() -> IssueKind {
            IssueKind::Configuration(ConfigurationIssue::OnlyGithubTeamOwnerAllowed)
        }

        pub fn single_owner_only() -> IssueKind {
            IssueKind::Configuration(ConfigurationIssue::OnlyOneOwnerPerEntry)
        }
    }
}
