// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::ConsistencyIssue;
use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};

pub trait CheckGithubConsistency {
    async fn github_identity(&self, identity: &GithubIdentityHandle) -> Result<(), ConsistencyIssue>;

    async fn github_team(&self, team: &GithubTeamHandle) -> Result<(), ConsistencyIssue>;
}

pub enum GithubConsistencyChecker {
    ApiBased,
    #[cfg(test)]
    FakeChecks(FakeGithubState),
    #[cfg(test)]
    ConsistentState,
}

#[allow(unused_variables)]
impl CheckGithubConsistency for GithubConsistencyChecker {
    async fn github_identity(&self, identity: &GithubIdentityHandle) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased => Ok(()),
            #[cfg(test)]
            GithubConsistencyChecker::FakeChecks(fake) => {
                if fake.known_users.contains(&identity.inner().to_string()) {
                    return Ok(());
                };

                Err(ConsistencyIssue::UserDoesNotBelongToOrganization(identity.clone()))
            },
            #[cfg(test)]
            GithubConsistencyChecker::ConsistentState => Ok(()),
        }
    }

    async fn github_team(&self, team: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased => Ok(()),
            #[cfg(test)]
            GithubConsistencyChecker::FakeChecks(state) => {
                let formatted = format!("{}/{}", &team.organization.inner(), &team.name);
                if state.known_teams.contains(&formatted) {
                    return Ok(());
                };

                Err(ConsistencyIssue::TeamDoesNotExistWithinOrganization(team.clone()))
            },
            #[cfg(test)]
            GithubConsistencyChecker::ConsistentState => Ok(()),
        }
    }
}

#[cfg(test)]
pub struct FakeGithubState {
    known_users: Vec<String>,
    known_teams: Vec<String>,
}

#[cfg(test)]
#[derive(Default)]
pub struct FakeGithubStateBuilder {
    known_users: Vec<String>,
    known_teams: Vec<String>,
}

#[cfg(test)]
impl FakeGithubStateBuilder {
    pub fn add_known_user(mut self, username: &str) -> Self {
        self.known_users.push(username.replace("@", ""));
        self
    }

    pub fn add_known_team(mut self, team: &str) -> Self {
        self.known_teams.push(team.replace("@", ""));
        self
    }

    pub fn build(self) -> FakeGithubState {
        FakeGithubState::new(self.known_users, self.known_teams)
    }
}

#[cfg(test)]
impl FakeGithubState {
    pub fn builder() -> FakeGithubStateBuilder {
        FakeGithubStateBuilder::default()
    }

    fn new(known_users: Vec<String>, known_teams: Vec<String>) -> Self {
        Self {
            known_users,
            known_teams,
        }
    }
}
