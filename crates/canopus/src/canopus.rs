// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

pub mod validation;

use crate::canopus::validation::CodeOwnersValidator;
use crate::core::models::codeowners::CodeOwnersContext;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum CanopusCommand {
    ValidateCodeowners(PathBuf, String),
}

impl Display for CanopusCommand {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            CanopusCommand::ValidateCodeowners(_, _) => "Validates the CODEOWNERS configuration for a project",
        };

        formatter.write_str(formatted)
    }
}

pub struct Canopus {
    codeowners_validator: CodeOwnersValidator,
}

impl Canopus {
    pub async fn execute(&self, requested: CanopusCommand) -> anyhow::Result<()> {
        match requested {
            CanopusCommand::ValidateCodeowners(project_path, organization_name) => {
                let codeowners_attributes = CodeOwnersContext::try_from(project_path)?;
                self.codeowners_validator
                    .validate_codeowners(codeowners_attributes, &organization_name)
                    .await
            },
        }
    }

    pub fn new(codeowners_validator: CodeOwnersValidator) -> Self {
        Self { codeowners_validator }
    }
}
