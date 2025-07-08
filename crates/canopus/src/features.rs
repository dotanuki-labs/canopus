// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

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
            validation::validate_codeowners(codeowners_file)
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::core::errors::{CodeownersValidationError, ValidationDiagnostic};
    use crate::core::models::CodeOwnersFile;
    use crate::features::validation;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;
    use std::path::PathBuf;

    #[test]
    fn should_find_no_issues() {
        let entries = indoc! {"
            *.rs    @org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file);

        assertor::assert_that!(validation).is_ok();
    }

    #[test]
    fn should_detect_invalid_owners() {
        let entries = indoc! {"
            *.rs    org/rustaceans
        "};

        let codeowners_file = CodeOwnersFile {
            path: PathBuf::from("path/to/.github/CODEOWNERS"),
            contents: entries.to_string(),
        };

        let validation = validation::validate_codeowners(codeowners_file);

        let expected = CodeownersValidationError {
            diagnostics: vec![ValidationDiagnostic::new_syntax_issue(0, "cannot parse owner")],
        };

        assertor::assert_that!(validation.unwrap_err().downcast_ref()).is_equal_to(Some(&expected));
    }
}
