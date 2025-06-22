// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod validation;

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
        RequestedFeature::ValidateCodeowners(project_path) => validation::validate_codeowners(&project_path),
    }
}
