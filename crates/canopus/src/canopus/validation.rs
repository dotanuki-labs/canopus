// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::models::codeowners::{CodeOwners, CodeOwnersContext, CodeOwnersEntry};
use crate::core::models::config::CanopusConfig;
use crate::core::models::handles::Owner;
use crate::core::models::{
    ConfigurationIssue, ConsistencyIssue, IssueKind, StructuralIssue, ValidationIssue, ValidationOutcome,
};
use crate::infra::github::{CheckGithubConsistency, GithubConsistencyChecker};
use crate::infra::paths::{DirWalking, PathWalker};
use itertools::Itertools;
use std::collections::HashSet;
use std::path::PathBuf;

pub struct CodeOwnersValidator {
    github_consistency_checker: GithubConsistencyChecker,
    path_walker: PathWalker,
}

impl CodeOwnersValidator {
    pub fn new(github_consistency_checker: GithubConsistencyChecker, path_walker: PathWalker) -> Self {
        Self {
            github_consistency_checker,
            path_walker,
        }
    }

    pub async fn validate(
        &self,
        codeowners_context: &CodeOwnersContext,
        canopus_config: &CanopusConfig,
    ) -> anyhow::Result<ValidationOutcome> {
        let project_root = codeowners_context.project_path.as_path();
        let codeowners = CodeOwners::try_from(codeowners_context.contents.as_str())?;
        log::info!("Syntax errors : not found");

        let gh_org = canopus_config.general.github_organization.as_str();

        let validations = vec![
            codeowners.syntax_validation.clone(),
            self.check_non_matching_glob_patterns(&codeowners, &self.path_walker.walk(project_root))?,
            self.check_duplicated_owners(&codeowners)?,
            self.check_multiple_ownership_per_entry(&codeowners, canopus_config)?,
            self.check_allowed_owners(&codeowners, canopus_config)?,
            self.check_github_consistency(gh_org, &codeowners, canopus_config)
                .await?,
        ];

        if validations
            .iter()
            .all(|outcome| matches!(outcome, ValidationOutcome::NoIssues))
        {
            return Ok(ValidationOutcome::NoIssues);
        }

        let all_issues = validations
            .into_iter()
            .filter_map(|outcome| match outcome {
                ValidationOutcome::NoIssues => None,
                ValidationOutcome::IssuesDetected(issues) => Some(issues),
            })
            .flatten()
            .sorted_by_key(|issue| issue.line)
            .collect_vec();

        Ok(ValidationOutcome::IssuesDetected(all_issues))
    }

    fn check_multiple_ownership_per_entry(
        &self,
        code_owners: &CodeOwners,
        canopus_config: &CanopusConfig,
    ) -> anyhow::Result<ValidationOutcome> {
        if !canopus_config.ownership.enforce_one_owner_per_line.unwrap_or(false) {
            return Ok(ValidationOutcome::NoIssues);
        };

        let entries_with_multiple_owners = code_owners
            .entries
            .iter()
            .filter_map(|entry| match entry {
                CodeOwnersEntry::Rule(ownership) => {
                    if ownership.owners.len() != 1 {
                        Some(ownership)
                    } else {
                        None
                    }
                },
                _ => None,
            })
            .collect_vec();

        if entries_with_multiple_owners.is_empty() {
            log::info!("All ownership entries have a single owner defined");
            return Ok(ValidationOutcome::NoIssues);
        };

        let issues = entries_with_multiple_owners
            .iter()
            .map(|rule| {
                ValidationIssue::builder()
                    .kind(IssueKind::Configuration(
                        ConfigurationIssue::OnlyOneOwnerPerEntryAllowed,
                    ))
                    .line_number(rule.line_number)
                    .description("Entry defines more than one owner for this glob")
                    .build()
            })
            .collect_vec();

        log::info!("Found some CodeOwners entries with multiple owners for the same glob");
        Ok(ValidationOutcome::IssuesDetected(issues))
    }

    fn check_duplicated_owners(&self, code_owners: &CodeOwners) -> anyhow::Result<ValidationOutcome> {
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
            let issues = grouped_per_glob
                .iter()
                .map(|(glob, lines)| {
                    ValidationIssue::builder()
                        .kind(IssueKind::Structural(StructuralIssue::DuplicateOwnership))
                        .line_number(lines[0])
                        .message(format!("{} defined multiple times : lines {:?}", glob, lines))
                        .build()
                })
                .collect_vec();

            log::info!("Found some duplicated ownership rules");
            return Ok(ValidationOutcome::IssuesDetected(issues));
        }

        log::info!("Duplicated code owners : not found");
        Ok(ValidationOutcome::NoIssues)
    }

    fn check_non_matching_glob_patterns(
        &self,
        code_owners: &CodeOwners,
        paths: &[PathBuf],
    ) -> anyhow::Result<ValidationOutcome> {
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

        let issues = lines_and_glob_matchers
            .iter()
            .filter(|(_, glob_matcher)| !matching_globs.contains(glob_matcher.glob()))
            .map(|(line, glob_matcher)| {
                ValidationIssue::builder()
                    .kind(IssueKind::Structural(StructuralIssue::DanglingGlobPattern))
                    .line_number(*line)
                    .message(format!("{} does not match any project path", glob_matcher.glob()))
                    .build()
            })
            .collect_vec();

        if !issues.is_empty() {
            log::info!("Found patterns that won't match any existing project files");
            return Ok(ValidationOutcome::IssuesDetected(issues));
        }

        log::info!("Dangling glob patterns : not found");
        Ok(ValidationOutcome::NoIssues)
    }

    fn check_allowed_owners(
        &self,
        code_owners: &CodeOwners,
        canopus_config: &CanopusConfig,
    ) -> anyhow::Result<ValidationOutcome> {
        if canopus_config.ownership.enforce_github_teams_owners.unwrap_or(false) {
            return self.check_only_github_teams_owners(code_owners);
        };

        if canopus_config.ownership.forbid_email_owners.unwrap_or(false) {
            return self.check_non_email_owners(code_owners);
        };

        Ok(ValidationOutcome::NoIssues)
    }

    fn check_non_email_owners(&self, code_owners: &CodeOwners) -> anyhow::Result<ValidationOutcome> {
        let email_owners = code_owners
            .unique_owners()
            .into_iter()
            .filter(|owner| matches!(owner, Owner::EmailAddress(_)))
            .collect_vec();

        if email_owners.is_empty() {
            log::info!("Email owners : not found");
            return Ok(ValidationOutcome::NoIssues);
        };

        let issues = email_owners
            .into_iter()
            .map(|owner| {
                ValidationIssue::builder()
                    .kind(IssueKind::Configuration(ConfigurationIssue::EmailOwnerNotAllowed))
                    .line_number(code_owners.occurrences(owner)[0])
                    .message("email owner is not allowed".to_string())
                    .build()
            })
            .collect_vec();

        log::info!("Found owners defined by email");
        Ok(ValidationOutcome::IssuesDetected(issues))
    }

    fn check_only_github_teams_owners(&self, code_owners: &CodeOwners) -> anyhow::Result<ValidationOutcome> {
        let non_github_team_owners = code_owners
            .unique_owners()
            .into_iter()
            .filter(|owner| !matches!(owner, Owner::GithubTeam(_)))
            .collect_vec();

        if non_github_team_owners.is_empty() {
            log::info!("All owners are Github teams");
            return Ok(ValidationOutcome::NoIssues);
        };

        let issues = non_github_team_owners
            .into_iter()
            .map(|owner| {
                ValidationIssue::builder()
                    .kind(IssueKind::Configuration(ConfigurationIssue::OnlyGithubTeamOwnerAllowed))
                    .line_number(code_owners.occurrences(owner)[0])
                    .message("only github team owner is allowed".to_string())
                    .build()
            })
            .collect_vec();

        log::info!("Found owners defined by email or github users");
        Ok(ValidationOutcome::IssuesDetected(issues))
    }

    async fn check_github_consistency(
        &self,
        organization: &str,
        code_owners: &CodeOwners,
        canopus_config: &CanopusConfig,
    ) -> anyhow::Result<ValidationOutcome> {
        let offline_checks_only = canopus_config.general.offline_checks_only.unwrap_or(false);

        if offline_checks_only {
            return Ok(ValidationOutcome::NoIssues);
        }

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
            return Ok(ValidationOutcome::NoIssues);
        };

        let issues = consistency_results
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
                ConsistencyIssue::TeamDoesNotExist(handle) => {
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
                ConsistencyIssue::OutsiderUser(handle) => {
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
                ConsistencyIssue::TeamDoesNotMatchOrganization(handle) => {
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
                ValidationIssue::builder()
                    .kind(IssueKind::Consistency(issue))
                    .line_number(line)
                    .message(cause)
                    .build()
            })
            .collect_vec();

        Ok(ValidationOutcome::IssuesDetected(issues))
    }
}

#[cfg(test)]
mod test_builders {
    use crate::canopus::validation::CodeOwnersValidator;
    use crate::core::models::codeowners::CodeOwnersContext;
    use crate::core::models::config;
    use crate::core::models::config::CanopusConfig;
    use crate::infra::github::{FakeGithubState, GithubConsistencyChecker};
    use crate::infra::paths::PathWalker;
    use std::path::PathBuf;

    pub fn codeowners_attributes(contents: &str) -> CodeOwnersContext {
        CodeOwnersContext {
            project_path: PathBuf::from("/usr/projects/my-project"),
            codeowners_path: PathBuf::from("/usr/projects/my-project/.github/CODEOWNERS"),
            contents: contents.to_string(),
        }
    }

    pub fn simple_canopus_config(github_organization: &str) -> CanopusConfig {
        CanopusConfig {
            general: config::General {
                github_organization: github_organization.to_string(),
                ..Default::default()
            },
            ..Default::default()
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

    pub fn panics_for_online_checks_validator(project_paths: Vec<&str>) -> CodeOwnersValidator {
        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::AlwaysPanic;
        CodeOwnersValidator::new(consistency_checker, path_walker)
    }
}

#[cfg(test)]
mod structural_validation_tests {
    use crate::canopus::validation::test_builders;
    use crate::core::models::test_helpers::ValidationIssueKindFactory;
    use crate::core::models::{ValidationIssue, ValidationOutcome};
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_detect_owners_syntax_issue() {
        let contents = indoc! {"
            *.rs    org/rustaceans
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let issue = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::invalid_syntax())
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![issue]);

        assertor::assert_that!(validation).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_glob_syntax_issue() {
        let contents = indoc! {"
            [z-a]*.rs    @org/crabbers
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let issue = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::invalid_syntax())
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![issue]);

        assertor::assert_that!(validation).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_report_multiple_issues_for_the_same_entry() {
        let contents = indoc! {"
            [z-a]*.rs    org/crabbers
        "};

        let context = test_builders::codeowners_attributes(contents);
        let validator = test_builders::structural_only_codeowners_validator(vec![]);

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let invalid_glob = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::invalid_syntax())
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let invalid_owner = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::invalid_syntax())
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let issues = vec![invalid_glob, invalid_owner];
        let expected = ValidationOutcome::IssuesDetected(issues);

        assertor::assert_that!(validation).is_equal_to(expected);
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let issue = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description(".automation/** does not match any project path")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![issue]);

        assertor::assert_that!(validation).is_equal_to(expected);
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let duplicated_ownership = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::duplicate_ownership())
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![duplicated_ownership]);

        assertor::assert_that!(validation).is_equal_to(expected);
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let dangling_glob = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description("docs/**/*.md does not match any project path")
            .build();

        let duplicated_ownership = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::duplicate_ownership())
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let issues = vec![duplicated_ownership, dangling_glob];
        let expected = ValidationOutcome::IssuesDetected(issues);
        assertor::assert_that!(validation).is_equal_to(expected);
    }
}

#[cfg(test)]
mod consistency_validation_tests {
    use crate::canopus::validation::test_builders;
    use crate::core::models::test_helpers::ValidationIssueKindFactory;
    use crate::core::models::{ValidationIssue, ValidationOutcome};
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await;

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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let user_not_found = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::user_does_not_belong_to_organization("ufs"))
            .line_number(1)
            .description("'ufs' user does not belong to this organization")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![user_not_found]);
        assertor::assert_that!(validation).is_equal_to(expected);
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

        let config = test_builders::simple_canopus_config("dotanuki-labs");

        let validation = validator.validate(&context, &config).await.unwrap();

        let user_not_found = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::team_does_not_exist(
                "dotanuki-labs",
                "devops",
            ))
            .line_number(2)
            .description("'devops' team does not belong to 'dotanuki-labs' organization")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![user_not_found]);
        assertor::assert_that!(validation).is_equal_to(expected);
    }
}

#[cfg(test)]
mod configuration_aware_tests {
    use crate::canopus::validation::test_builders;
    use crate::core::models::config::{CanopusConfig, Ownership};
    use crate::core::models::test_helpers::ValidationIssueKindFactory;
    use crate::core::models::{ValidationIssue, ValidationOutcome, config};
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;

    #[tokio::test]
    async fn should_honor_offline_checks_only() {
        let contents = indoc! {"
            *.rs    @org/rustaceans
        "};

        let project_paths = vec!["main.rs"];

        let context = test_builders::codeowners_attributes(contents);

        // Forces panic if any Github consistency checks are used
        let validator = test_builders::panics_for_online_checks_validator(project_paths);

        let config = CanopusConfig {
            general: config::General {
                github_organization: "dotanuki-labs".to_string(),
                offline_checks_only: Some(true),
            },
            ..Default::default()
        };

        let validation = validator.validate(&context, &config).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_deny_email_owners() {
        let contents = indoc! {"
            *.rs    me@hakagi.dev
        "};

        let project_paths = vec!["main.rs"];

        let context = test_builders::codeowners_attributes(contents);

        // Forces panic if any Github consistency checks are used
        let validator = test_builders::panics_for_online_checks_validator(project_paths);

        let config = CanopusConfig {
            general: config::General {
                github_organization: "dotanuki-labs".to_string(),
                offline_checks_only: Some(true),
            },
            ownership: Ownership {
                forbid_email_owners: Some(true),
                ..Default::default()
            },
        };

        let validation = validator.validate(&context, &config).await.unwrap();

        let email_owner_not_allowed = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::github_owners_only())
            .line_number(0)
            .description("email owner is not allowed")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![email_owner_not_allowed]);
        assertor::assert_that!(validation).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_enforce_github_teams_owners() {
        let contents = indoc! {"
            *.rs    @ubiratansoares
        "};

        let project_paths = vec!["main.rs"];

        let context = test_builders::codeowners_attributes(contents);

        // Forces panic if any Github consistency checks are used
        let validator = test_builders::panics_for_online_checks_validator(project_paths);

        let config = CanopusConfig {
            general: config::General {
                github_organization: "dotanuki-labs".to_string(),
                offline_checks_only: Some(true),
            },
            ownership: Ownership {
                enforce_github_teams_owners: Some(true),
                ..Default::default()
            },
        };

        let validation = validator.validate(&context, &config).await.unwrap();

        let only_team_owner_allowed = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::github_team_owners_only())
            .line_number(0)
            .description("only github team owner is allowed")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![only_team_owner_allowed]);
        assertor::assert_that!(validation).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_enforce_single_owner_per_entry() {
        let contents = indoc! {"
            *.rs    @ubiratansoares @dotanukibot
        "};

        let project_paths = vec!["main.rs"];

        let context = test_builders::codeowners_attributes(contents);

        // Forces panic if any Github consistency checks are used
        let validator = test_builders::panics_for_online_checks_validator(project_paths);

        let config = CanopusConfig {
            general: config::General {
                github_organization: "dotanuki-labs".to_string(),
                offline_checks_only: Some(true),
            },
            ownership: Ownership {
                enforce_one_owner_per_line: Some(true),
                ..Default::default()
            },
        };

        let validation = validator.validate(&context, &config).await.unwrap();

        let only_one_owner_allowed = ValidationIssue::builder()
            .kind(ValidationIssueKindFactory::single_owner_only())
            .line_number(0)
            .description("Entry defines more than one owner for this glob")
            .build();

        let expected = ValidationOutcome::IssuesDetected(vec![only_one_owner_allowed]);
        assertor::assert_that!(validation).is_equal_to(expected);
    }
}
