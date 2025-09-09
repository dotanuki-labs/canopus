// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::{
    CodeownersValidationError, ConsistencyIssue, DiagnosticKind, StructuralIssue, ValidationDiagnostic,
};
use crate::core::models::codeowners::{CodeOwners, CodeOwnersContext, CodeOwnersEntry};
use crate::core::models::config::CanopusConfig;
use crate::core::models::handles::Owner;
use crate::infra::github::{CheckGithubConsistency, GithubConsistencyChecker};
use crate::infra::paths::{DirWalking, PathWalker};
use anyhow::bail;
use itertools::Itertools;
use std::collections::HashSet;
use std::path::PathBuf;

pub struct CodeOwnersValidator {
    github_consistency_checker: GithubConsistencyChecker,
    path_walker: PathWalker,
}

impl CodeOwnersValidator {
    pub async fn validate_codeowners(
        &self,
        codeowners_file: CodeOwnersContext,
        canopus_config: CanopusConfig,
    ) -> anyhow::Result<()> {
        let project_root = codeowners_file.project_root.as_path();
        let codeowners = CodeOwners::try_from(codeowners_file.contents.as_str())?;
        log::info!("Syntax errors : not found");

        let validations = vec![
            self.check_non_matching_glob_patterns(&codeowners, &self.path_walker.walk(project_root)),
            self.check_duplicated_owners(&codeowners),
            self.check_github_consistency(&canopus_config.github_organization, &codeowners)
                .await,
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

        bail!(CodeownersValidationError::with(diagnostics));
    }

    fn check_duplicated_owners(&self, code_owners: &CodeOwners) -> anyhow::Result<()> {
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
                        .kind(DiagnosticKind::Structural(StructuralIssue::DuplicateOwnership))
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

    fn check_non_matching_glob_patterns(&self, code_owners: &CodeOwners, paths: &[PathBuf]) -> anyhow::Result<()> {
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
                    .kind(DiagnosticKind::Structural(StructuralIssue::DanglingGlobPattern))
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

    async fn check_github_consistency(&self, organization: &str, code_owners: &CodeOwners) -> anyhow::Result<()> {
        let unique_ownerships = code_owners.unique_owners();

        let consistency_checks = unique_ownerships
            .into_iter()
            .map(|owner| async move {
                match owner {
                    Owner::GithubUser(identity) => {
                        self.github_consistency_checker
                            .github_identity(organization, identity)
                            .await
                    },
                    Owner::GithubTeam(team) => self.github_consistency_checker.github_team(organization, team).await,
                    Owner::EmailAddress(_) => Ok(()),
                }
            })
            .collect_vec();

        let consistency_results = futures::future::join_all(consistency_checks).await;

        if consistency_results.iter().all(|check| check.is_ok()) {
            return Ok(());
        };

        let diagnostics = consistency_results
            .into_iter()
            .filter_map(|check| check.err())
            .map(|issue| match issue.clone() {
                ConsistencyIssue::UserDoesNotExist(handle) => {
                    let owner = Owner::GithubUser(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!("'{}' user does not exist", handle.inner()),
                    )
                },
                ConsistencyIssue::OrganizationDoesNotExist(handle) => {
                    let owner = Owner::GithubUser(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!("'{}' organization does not exist", handle.inner()),
                    )
                },
                ConsistencyIssue::TeamDoesNotExistWithinOrganization(handle) => {
                    let owner = Owner::GithubTeam(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!(
                            "'{}' team does not belong to '{}' organization",
                            handle.name,
                            handle.organization.inner()
                        ),
                    )
                },
                ConsistencyIssue::UserDoesNotBelongToOrganization(handle) => {
                    let owner = Owner::GithubUser(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!("'{}' user does not belong to this organization", handle.inner()),
                    )
                },
                ConsistencyIssue::CannotVerifyUser(handle) => {
                    let owner = Owner::GithubUser(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!("cannot confirm if user '{}' exists", handle.inner()),
                    )
                },
                ConsistencyIssue::CannotVerifyTeam(handle) => {
                    let owner = Owner::GithubTeam(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!(
                            "cannot confirm whether '{}/{}' team exists",
                            handle.organization.inner(),
                            handle.name
                        ),
                    )
                },
                ConsistencyIssue::CannotListMembersInTheOrganization(organization) => (
                    issue,
                    usize::MAX,
                    format!("failed to list members that belong to '{}' organization", organization),
                ),
                ConsistencyIssue::TeamDoesNotMatchWithProvidedOrganization(handle) => {
                    let owner = Owner::GithubTeam(handle.clone());
                    let first_occurrence = code_owners.occurrences(&owner)[0];
                    (
                        issue,
                        first_occurrence,
                        format!(
                            "team '{}/{}' does not belong to this organization",
                            handle.organization.inner(),
                            handle.name
                        ),
                    )
                },
            })
            .map(|(issue, line, cause)| {
                ValidationDiagnostic::builder()
                    .kind(DiagnosticKind::Consistency(issue))
                    .line_number(line)
                    .message(cause)
                    .build()
            })
            .collect_vec();

        bail!(CodeownersValidationError::with(diagnostics))
    }

    pub fn new(github_consistency_checker: GithubConsistencyChecker, path_walker: PathWalker) -> Self {
        Self {
            github_consistency_checker,
            path_walker,
        }
    }
}

#[cfg(test)]
mod test_builders {
    use crate::canopus::validation::CodeOwnersValidator;
    use crate::core::models::codeowners::CodeOwnersContext;
    use crate::infra::github::{FakeGithubState, GithubConsistencyChecker};
    use crate::infra::paths::PathWalker;
    use std::path::PathBuf;

    pub fn codeowners_attributes(contents: &str) -> CodeOwnersContext {
        CodeOwnersContext {
            project_root: PathBuf::from("/usr/projects/my-project"),
            location: PathBuf::from("/usr/projects/my-project/.github/CODEOWNERS"),
            contents: contents.to_string(),
        }
    }

    pub fn structural_only_codeowners_validator(project_paths: Vec<&str>) -> CodeOwnersValidator {
        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        CodeOwnersValidator::new(consistency_checker, path_walker)
    }

    pub fn consistency_aware_codeowners_validator(
        project_paths: Vec<&str>,
        state: FakeGithubState,
    ) -> CodeOwnersValidator {
        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::FakeChecks(state);
        CodeOwnersValidator::new(consistency_checker, path_walker)
    }
}

#[cfg(test)]
mod structural_validation_tests {
    use crate::canopus::validation::test_builders;
    use crate::core::errors::test_helpers::DiagnosticKindFactory;
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
    use crate::core::models::config::CanopusConfig;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;

    #[tokio::test]
    async fn should_find_no_syntax_issues() {
        let contents = indoc! {"
            *.rs    @org/rustaceans
        "};

        let project_paths = vec!["main.rs"];

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(project_paths);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_detect_owners_syntax_issue() {
        let contents = indoc! {"
            *.rs    org/rustaceans
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::invalid_syntax())
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_glob_syntax_issue() {
        let contents = indoc! {"
            [z-a]*.rs    @org/crabbers
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::invalid_syntax())
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_report_multiple_issues_for_the_same_entry() {
        let contents = indoc! {"
            [z-a]*.rs    org/crabbers
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let invalid_glob = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::invalid_syntax())
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let invalid_owner = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::invalid_syntax())
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let expected = CodeownersValidationError::with(vec![invalid_glob, invalid_owner]);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_dangling_globs() {
        let contents = indoc! {"
            *.rs                @dotanuki-labs/rustaceans
            .automation/**      @dotanuki-labs/infra
        "};

        let project_paths = vec!["validation.rs"];

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(project_paths);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description(".automation/** does not match any project path")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_strictly_duplicated_ownership_rules() {
        let contents = indoc! {"
            *.rs            @org/rustaceans
            docs/**/*.md    @org/devs
            *.rs            @org/crabbers @ubiratansoares
        "};

        let project_paths = vec!["validation.rs", "docs/README.md", "docs/README.md"];

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(project_paths);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let duplicated_ownership = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::duplicate_ownership())
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let expected = CodeownersValidationError::from(duplicated_ownership);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_multiple_non_syntax_issues() {
        let contents = indoc! {"
            *.rs            @org/rustaceans
            docs/**/*.md    @org/devs
            *.rs            @org/crabbers @ubiratansoares
        "};

        let project_paths = vec!["validation.rs", ".github", ".github/CODEOWNERS"];

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(project_paths);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let dangling_glob = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description("docs/**/*.md does not match any project path")
            .build();

        let duplicated_ownership = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::duplicate_ownership())
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let expected = CodeownersValidationError::with(vec![dangling_glob, duplicated_ownership]);
        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }
}

#[cfg(test)]
mod consistency_validation_tests {
    use crate::canopus::validation::test_builders;
    use crate::core::errors::test_helpers::DiagnosticKindFactory;
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
    use crate::core::models::config::CanopusConfig;
    use crate::infra::github;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;

    #[tokio::test]
    async fn should_find_no_consistency_issues() {
        let contents = indoc! {"
            *.rs            @dotanuki-labs/rustaceans
            .github/**/*    @ubiratansoares
        "};

        let project_paths = vec![".github/CODEOWNERS", "main.rs"];

        let github_state = github::FakeGithubState::builder()
            .add_known_user("@ubiratansoares")
            .add_known_team("@dotanuki-labs/rustaceans")
            .build();

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::consistency_aware_codeowners_validator(project_paths, github_state);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_detect_non_existing_github_user() {
        let contents = indoc! {"
            *.rs            @dotanuki-labs/rustaceans
            .github/**/*    @ufs
        "};

        let project_paths = vec![".github/CODEOWNERS", "main.rs"];

        let github_state = github::FakeGithubState::builder()
            .add_known_team("@dotanuki-labs/rustaceans")
            .build();

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::consistency_aware_codeowners_validator(project_paths, github_state);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let user_not_found = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::user_does_not_belong_to_organization("ufs"))
            .line_number(1)
            .description("'ufs' user does not belong to this organization")
            .build();

        let expected = CodeownersValidationError::from(user_not_found);
        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_non_existing_github_team() {
        let contents = indoc! {"
            *.rs            @dotanuki-labs/rustaceans
            *.md            @dotanuki-labs/writers
            .github/*.json  @dotanuki-labs/devops
        "};

        let project_paths = vec![".github/renovate.json", "README.md", "main.rs"];

        let github_state = github::FakeGithubState::builder()
            .add_known_team("@dotanuki-labs/rustaceans")
            .add_known_team("@dotanuki-labs/writers")
            .build();

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::consistency_aware_codeowners_validator(project_paths, github_state);

        let config = CanopusConfig::new("dotanuki-labs");
        let validation = validator.validate_codeowners(context, config).await;

        let user_not_found = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::team_does_not_exist("dotanuki-labs", "devops"))
            .line_number(2)
            .description("'devops' team does not belong to 'dotanuki-labs' organization")
            .build();

        let expected = CodeownersValidationError::from(user_not_found);
        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }
}
