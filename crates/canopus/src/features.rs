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
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
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

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_op());

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

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_op());

        let expected = CodeownersValidationError {
            diagnostics: vec![ValidationDiagnostic::new_syntax_issue(0, "cannot parse owner")],
        };

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

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_op());

        let expected = CodeownersValidationError {
            diagnostics: vec![ValidationDiagnostic::new_syntax_issue(0, "invalid glob pattern")],
        };

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

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_op());

        let expected = CodeownersValidationError {
            diagnostics: vec![
                ValidationDiagnostic::new_syntax_issue(0, "invalid glob pattern"),
                ValidationDiagnostic::new_syntax_issue(0, "cannot parse owner"),
            ],
        };

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }

    #[test]
    fn should_detect_strictly_duplicated_ownership_rules() {
        let entries = indoc! {"
            *.rs    @org/rustaceans
            .github @org/devops
            *.rs    @org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file, FakePathWalker::no_op());

        let expected = CodeownersValidationError {
            diagnostics: vec![ValidationDiagnostic::new_duplicated_ownership(
                1,
                "*.rs defined multiple times : lines [1, 3]",
            )],
        };

        assertor::assert_that!(validation.into()).is_equal_to(expected);
    }
}
