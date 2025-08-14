// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod filesystem;
mod validation;

use crate::core::models::CodeOwnersFile;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum RequestedFeature {
    ValidateCodeowners(PathBuf),
}

impl Display for RequestedFeature {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            RequestedFeature::ValidateCodeowners(_) => "Validates the CODEOWNERS configuration for a project",
        };

        formatter.write_str(formatted)
    }
}
pub fn execute(requested: RequestedFeature) -> anyhow::Result<()> {
    match requested {
        RequestedFeature::ValidateCodeowners(project_path) => {
            let codeowners_file = CodeOwnersFile::try_from(project_path.clone())?;
            let path_walker = filesystem::GitAwarePathWalker::new(codeowners_file.path.clone());
            validation::validate_codeowners(codeowners_file, path_walker)
        },
    }
}

#[cfg(test)]
mod validation_tests {
    use crate::core::errors::{CodeownersValidationError, DiagnosticKind, ValidationDiagnostic};
    use crate::core::models::CodeOwnersFile;
    use crate::features::filesystem::helpers::FakePathWalker;
    use crate::features::validation;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;
    use std::path::PathBuf;

    #[test]
    fn should_find_no_syntax_issues() {
        let entries = indoc! {"
            *.rs    @org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = ["main.rs"];

        let path_walker = FakePathWalker::new(&project_paths);

        let validation = validation::validate_codeowners(codeowners_file, path_walker);

        assertor::assert_that!(validation).is_ok();
    }

    #[test]
    fn should_detect_owners_syntax_issue() {
        let entries = indoc! {"
            *.rs    org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_walking());

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_detect_glob_syntax_issue() {
        let entries = indoc! {"
            [z-a]*.rs    @org/crabbers
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_walking());

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_report_multiple_issues_for_the_same_entry() {
        let entries = indoc! {"
            [z-a]*.rs    org/crabbers
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_walking());

        let invalid_glob = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(0)
            .description("invalid glob pattern")
            .build();

        let invalid_owner = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::InvalidSyntax)
            .line_number(0)
            .description("cannot parse owner")
            .build();

        let expected = CodeownersValidationError::with(vec![invalid_glob, invalid_owner]);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_detect_dangling_globs() {
        let entries = indoc! {"
            *.rs            @org/rustaceans
            .automation/    @org/infra
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = [
            // Already present as a project file
            "validation.rs",
        ];

        let path_walker = FakePathWalker::new(&project_paths);

        let validation = validation::validate_codeowners(codeowners_file, path_walker);

        let issue = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::DanglingGlobPattern)
            .line_number(1)
            .description(".automation/ does not match any project path")
            .build();

        let expected = CodeownersValidationError::from(issue);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_detect_strictly_duplicated_ownership_rules() {
        let entries = indoc! {"
            *.rs     @org/rustaceans
            docs/    @org/rustaceans
            *.rs     @org/crabbers @ubiratansoares
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = ["validation.rs", "docs/", "docs/README.md"];

        let path_walker = FakePathWalker::new(&project_paths);

        let validation = validation::validate_codeowners(codeowners_file, path_walker);

        let duplicated_ownership = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::DuplicateOwnership)
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let expected = CodeownersValidationError::from(duplicated_ownership);

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_detect_multiple_non_syntax_issues() {
        let entries = indoc! {"
            *.rs        @org/rustaceans
            docs/       @org/devs
            *.rs        @org/crabbers @ubiratansoares
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let project_paths = ["validation.rs", ".github/", ".github/CODEOWNERS"];

        let path_walker = FakePathWalker::new(&project_paths);

        let validation = validation::validate_codeowners(codeowners_file, path_walker);

        let dangling_glob = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::DanglingGlobPattern)
            .line_number(1)
            .description("docs/ does not match any project path")
            .build();

        let duplicated_ownership = ValidationDiagnostic::builder()
            .kind(DiagnosticKind::DuplicateOwnership)
            .line_number(0)
            .description("*.rs defined multiple times : lines [0, 2]")
            .build();

        let expected = CodeownersValidationError::with(vec![dangling_glob, duplicated_ownership]);
        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }
}
