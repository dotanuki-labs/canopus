// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use anyhow::bail;
use serde::Deserialize;
use std::path::Path;

/// Defaults for optional configuration values
pub static DEFAULT_VALUE_OFFLINE_CHECKS_ONLY: bool = false;
pub static DEFAULT_VALUE_ENFORCE_GITHUB_TEAMS_OWNERS: bool = false;
pub static DEFAULT_VALUE_ENFORCE_ONE_OWNER_PER_LINE: bool = false;
pub static DEFAULT_VALUE_FORBID_EMAIL_ADDRESSES: bool = false;

/// The configuration options for canopus
#[derive(Deserialize, Debug, Default)]
pub struct CanopusConfig {
    pub general: GeneralConfig,
    pub ownership: OwnershipConfig,
}

#[derive(Deserialize, Debug, Default)]
pub struct GeneralConfig {
    /// The Github organization that owns the target project
    #[serde(rename(deserialize = "github-organization"))]
    pub github_organization: String,

    /// Whether we should run verifications against Github API
    #[serde(rename(deserialize = "offline-checks-only"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_checks_only: Option<bool>,
}

#[derive(Deserialize, Debug, Default)]
pub struct OwnershipConfig {
    /// Whether we should enforce only Github teams as owners
    #[serde(rename(deserialize = "enforce-github-teams-owners"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_github_teams_owners: Option<bool>,

    /// Whether we should enforce one owner per Glob pattern
    #[serde(rename(deserialize = "enforce-one-owner-per-line"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_one_owner_per_line: Option<bool>,

    /// Whether we should accept an email address to identify an owner
    #[serde(rename(deserialize = "forbid-email-owners"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forbid_email_owners: Option<bool>,
}

/// Parsing the configuration file from a path
impl TryFrom<&Path> for CanopusConfig {
    type Error = anyhow::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let config_location = value.join(".github").join("canopus.toml");

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

#[cfg(test)]
mod tests {
    use crate::core::models::config::CanopusConfig;
    use assertor::StringAssertion;
    use temp_dir::TempDir;

    #[test]
    fn should_report_config_not_found() {
        let temp_dir = TempDir::new().expect("Cant create temp dir");

        let project_path = temp_dir.path().to_path_buf();

        let config = CanopusConfig::try_from(project_path.as_path());

        assertor::assert_that!(config.unwrap_err().to_string()).contains("expecting configuration at");
    }
}
