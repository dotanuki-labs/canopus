// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::models::codeowners::CodeOwnersContext;
use anyhow::bail;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CanopusConfig {
    #[serde(rename(deserialize = "github-organization"))]
    pub github_organization: String,
}

impl CanopusConfig {
    #[cfg(test)]
    pub fn new(organization: &str) -> Self {
        Self {
            github_organization: organization.into(),
        }
    }
}

impl TryFrom<&CodeOwnersContext> for CanopusConfig {
    type Error = anyhow::Error;

    fn try_from(value: &CodeOwnersContext) -> Result<Self, Self::Error> {
        let config_location = value.project_root.join(".github").join("canopus.toml");

        if !config_location.exists() {
            bail!("expecting configuration at : {}", config_location.display())
        }

        if !config_location.is_file() {
            bail!("expecting a file not a directory : {}", config_location.display())
        }

        log::debug!("Found canopus config at : {:?}", config_location);

        let contents = std::fs::read_to_string(config_location)?;
        let parsed = toml::from_str(&contents)?;
        Ok(parsed)
    }
}
