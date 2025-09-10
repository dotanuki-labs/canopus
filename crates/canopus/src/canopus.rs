// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

mod repairing;
pub mod validation;

use crate::canopus::validation::CodeOwnersValidator;
use crate::core::models::ValidationOutcome;
use crate::core::models::codeowners::CodeOwnersContext;
use crate::core::models::config::CanopusConfig;
use itertools::Itertools;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum CanopusCommand {
    ValidateCodeowners(PathBuf),
    RepairCodeowners {
        project_root: PathBuf,
        dry_run: bool,
        remove_lines: bool,
    },
}

impl Display for CanopusCommand {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            CanopusCommand::ValidateCodeowners(_) => "Validates the CODEOWNERS configuration for a project",
            CanopusCommand::RepairCodeowners { .. } => "Repairs the CODEOWNERS configuration for a project",
        };

        formatter.write_str(formatted)
    }
}

pub struct Canopus {
    codeowners_validator: CodeOwnersValidator,
}

impl Canopus {
    pub fn new(codeowners_validator: CodeOwnersValidator) -> Self {
        Self { codeowners_validator }
    }

    pub async fn execute(&self, requested: CanopusCommand) -> anyhow::Result<()> {
        match requested {
            CanopusCommand::ValidateCodeowners(project_path) => {
                let (context, config) = Self::evaluate(project_path)?;
                let outcome = self.codeowners_validator.validate(&context, &config).await?;

                match outcome {
                    ValidationOutcome::NoIssues => println!("No issues found"),
                    ValidationOutcome::IssuesDetected(issues) => {
                        issues.iter().for_each(|issue| {
                            println!("{}", issue);
                        });
                        println!("Some issues found")
                    },
                }
            },
            CanopusCommand::RepairCodeowners {
                project_root,
                dry_run,
                remove_lines,
            } => {
                let (context, config) = Self::evaluate(project_root)?;
                let outcome = self.codeowners_validator.validate(&context, &config).await?;

                match outcome {
                    ValidationOutcome::NoIssues => println!("Nothing to repair"),
                    ValidationOutcome::IssuesDetected(issues) => {
                        let unique_issues_per_line = issues.into_iter().unique_by(|issue| issue.line).collect_vec();

                        if dry_run {
                            println!("Dry-run repairing...");

                            unique_issues_per_line.iter().for_each(|issue| {
                                println!("L{} will be repaired ({})", issue.line + 1, issue.context);
                            });

                            println!();
                            println!("More issues can exist for every line above");
                            return Ok(());
                        }

                        println!("Repairing CodeOwners...");
                        let lines_to_repair = unique_issues_per_line.into_iter().map(|issue| issue.line).collect_vec();
                        repairing::repair_code_owners(&context, lines_to_repair, remove_lines)?
                    },
                }
            },
        }

        Ok(())
    }

    fn evaluate(project_path: PathBuf) -> anyhow::Result<(CodeOwnersContext, CanopusConfig)> {
        let codeowners_context = CodeOwnersContext::try_from(project_path.clone())?;
        let canopus_config = CanopusConfig::try_from(project_path.as_path())?;
        Ok((codeowners_context, canopus_config))
    }
}
