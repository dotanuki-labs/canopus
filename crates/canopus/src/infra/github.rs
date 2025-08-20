// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::ConsistencyIssue;
use crate::core::models::{GithubIdentityHandle, GithubTeamHandle};

pub trait GithubClient {
    fn check_github_identity(&self, identity: &GithubIdentityHandle) -> Result<(), ConsistencyIssue>;

    fn check_github_team(&self, team: &GithubTeamHandle) -> Result<(), ConsistencyIssue>;
}

pub struct GithubRestClient;

impl GithubRestClient {
    pub fn new() -> Self {
        Self {}
    }
}

impl GithubClient for GithubRestClient {
    fn check_github_identity(&self, _: &GithubIdentityHandle) -> Result<(), ConsistencyIssue> {
        Ok(())
    }

    fn check_github_team(&self, _: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
        Ok(())
    }
}

#[cfg(test)]
pub mod test_helpers {
    use crate::core::models::{GithubIdentityHandle, GithubTeamHandle};
    use crate::infra::github::{ConsistencyIssue, GithubClient};

    pub struct AllConsistentGithubClient;

    impl AllConsistentGithubClient {
        pub fn new() -> Self {
            Self {}
        }
    }

    impl GithubClient for AllConsistentGithubClient {
        fn check_github_identity(&self, _: &GithubIdentityHandle) -> Result<(), ConsistencyIssue> {
            Ok(())
        }

        fn check_github_team(&self, _: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
            Ok(())
        }
    }

    pub struct FakeGithubClient {
        known_users: Vec<String>,
        known_teams: Vec<String>,
    }

    #[derive(Default)]
    pub struct FakeGithubClientBuilder {
        known_users: Vec<String>,
        known_teams: Vec<String>,
    }

    impl FakeGithubClientBuilder {
        pub fn add_known_user(mut self, username: &str) -> Self {
            self.known_users.push(username.replace("@", ""));
            self
        }

        pub fn add_known_team(mut self, team: &str) -> Self {
            self.known_teams.push(team.replace("@", ""));
            self
        }

        pub fn build(self) -> FakeGithubClient {
            FakeGithubClient::new(self.known_users, self.known_teams)
        }
    }

    impl FakeGithubClient {
        pub fn builder() -> FakeGithubClientBuilder {
            FakeGithubClientBuilder::default()
        }

        fn new(known_users: Vec<String>, known_teams: Vec<String>) -> Self {
            Self {
                known_users,
                known_teams,
            }
        }
    }

    impl GithubClient for FakeGithubClient {
        fn check_github_identity(&self, identity: &GithubIdentityHandle) -> Result<(), ConsistencyIssue> {
            if self.known_users.contains(&identity.inner().to_string()) {
                return Ok(());
            };

            Err(ConsistencyIssue::UserDoesNotBelongToOrganization(identity.clone()))
        }

        fn check_github_team(&self, team: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
            let formatted = format!("{}/{}", &team.organization.inner(), &team.name);
            if self.known_teams.contains(&formatted) {
                return Ok(());
            };

            Err(ConsistencyIssue::TeamDoesNotExistWithinOrganization(team.clone()))
        }
    }
}
