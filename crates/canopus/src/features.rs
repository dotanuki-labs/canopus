// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod validation;

use crate::core::models::codeowners::CodeOwnersFile;
use crate::features::validation::CodeOwnersValidator;
use crate::infra::github::GithubConsistencyChecker;
use crate::infra::paths;
use octorust::Client;
use octorust::auth::Credentials;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum RequestedFeature {
    ValidateCodeowners(PathBuf, String),
}

impl Display for RequestedFeature {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            RequestedFeature::ValidateCodeowners(_, _) => "Validates the CODEOWNERS configuration for a project",
        };

        formatter.write_str(formatted)
    }
}

pub async fn execute(requested: RequestedFeature) -> anyhow::Result<()> {
    match requested {
        RequestedFeature::ValidateCodeowners(project_path, organization_name) => {
            let codeowners_file = CodeOwnersFile::try_from(project_path.clone())?;
            let path_walker = paths::PathWalker::GitAware(codeowners_file.path.clone());

            let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            let github_token = std::env::var("GITHUB_TOKEN").unwrap_or("github-pat".to_string());

            let consistency_checker = GithubConsistencyChecker::ApiBased {
                github_client: Client::new(user_agent, Credentials::Token(github_token))?,
                target_organization_name: organization_name,
            };

            let validator = CodeOwnersValidator::new(consistency_checker, path_walker);
            validator.validate_codeowners(codeowners_file).await
        },
    }
}

#[cfg(test)]
mod structural_validation_tests {
    use crate::core::errors::test_helpers::DiagnosticKindFactory;
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
    use crate::core::models::codeowners::CodeOwnersFile;
    use crate::features::validation::CodeOwnersValidator;
    use crate::infra::github;
    use crate::infra::paths::PathWalker;
    use assertor::{EqualityAssertion, ResultAssertion};
    use github::GithubConsistencyChecker;
    use indoc::indoc;
    use std::path::PathBuf;

    #[tokio::test]
    async fn should_find_no_syntax_issues() {
        let entries = indoc! {"
            *.rs    @org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec!["main.rs"];
        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_detect_owners_syntax_issue() {
        let entries = indoc! {"
            *.rs    org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let path_walker = PathWalker::with_paths(vec![]);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

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
        let entries = indoc! {"
            [z-a]*.rs    @org/crabbers
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let path_walker = PathWalker::with_paths(vec![]);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

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
        let entries = indoc! {"
            [z-a]*.rs    org/crabbers
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let path_walker = PathWalker::with_paths(vec![]);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

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
        let entries = indoc! {"
            *.rs            @org/rustaceans
            .automation/    @org/infra
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec!["validation.rs"];

        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description(".automation/ does not match any project path")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[tokio::test]
    async fn should_detect_strictly_duplicated_ownership_rules() {
        let entries = indoc! {"
            *.rs     @org/rustaceans
            docs/    @org/rustaceans
            *.rs     @org/crabbers @ubiratansoares
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec!["validation.rs", "docs/", "docs/README.md"];

        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

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
        let entries = indoc! {"
            *.rs        @org/rustaceans
            docs/       @org/devs
            *.rs        @org/crabbers @ubiratansoares
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec!["validation.rs", ".github/", ".github/CODEOWNERS"];
        let path_walker = PathWalker::with_paths(project_paths);
        let consistency_checker = GithubConsistencyChecker::ConsistentState;
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

        let dangling_glob = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::dangling_glob_pattern())
            .line_number(1)
            .description("docs/ does not match any project path")
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
    use crate::core::errors::test_helpers::DiagnosticKindFactory;
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
    use crate::core::models::codeowners::CodeOwnersFile;
    use crate::features::validation::CodeOwnersValidator;
    use crate::infra::github;
    use crate::infra::paths::PathWalker;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;
    use std::path::PathBuf;

    #[tokio::test]
    async fn should_find_no_consistency_issues() {
        let entries = indoc! {"
            *.rs        @dotanuki-labs/rustaceans
            .github/    @ubiratansoares
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec![".github/", "main.rs"];
        let path_walker = PathWalker::with_paths(project_paths);

        let github_state = github::FakeGithubState::builder()
            .add_known_user("@ubiratansoares")
            .add_known_team("@dotanuki-labs/rustaceans")
            .build();

        let consistency_checker = github::GithubConsistencyChecker::FakeChecks(github_state);
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

        assertor::assert_that!(validation).is_ok();
    }

    #[tokio::test]
    async fn should_detect_non_existing_github_user() {
        let entries = indoc! {"
            *.rs        @dotanuki-labs/rustaceans
            .github/    @ufs
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec![".github/", "main.rs"];
        let path_walker = PathWalker::with_paths(project_paths);

        let github_state = github::FakeGithubState::builder()
            .add_known_team("@dotanuki-labs/rustaceans")
            .build();

        let consistency_checker = github::GithubConsistencyChecker::FakeChecks(github_state);
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

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
        let entries = indoc! {"
            *.rs        @dotanuki-labs/rustaceans
            docs/       @dotanuki-labs/writers
            .github/    @dotanuki-labs/devops
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = vec![".github/", "docs/", "main.rs"];
        let path_walker = PathWalker::with_paths(project_paths);

        let github_state = github::FakeGithubState::builder()
            .add_known_team("@dotanuki-labs/rustaceans")
            .add_known_team("@dotanuki-labs/writers")
            .build();

        let consistency_checker = github::GithubConsistencyChecker::FakeChecks(github_state);
        let validator = CodeOwnersValidator::new(consistency_checker, path_walker);

        let validation = validator.validate_codeowners(codeowners_file).await;

        let user_not_found = ValidationDiagnostic::builder()
            .kind(DiagnosticKindFactory::team_does_not_exist("dotanuki-labs", "devops"))
            .line_number(2)
            .description("'devops' team does not belong to 'dotanuki-labs' organization")
            .build();

        let expected = CodeownersValidationError::from(user_not_found);
        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }
}
