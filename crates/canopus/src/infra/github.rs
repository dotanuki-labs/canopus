// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::ConsistencyIssue;
use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
use octorust::{ClientError, StatusCode};

pub trait CheckGithubConsistency {
    async fn github_identity(&self, handle: &GithubIdentityHandle) -> Result<(), ConsistencyIssue>;

    async fn github_team(&self, handle: &GithubTeamHandle) -> Result<(), ConsistencyIssue>;
}

pub enum GithubConsistencyChecker {
    ApiBased(octorust::Client),
    #[cfg(test)]
    FakeChecks(FakeGithubState),
    #[cfg(test)]
    ConsistentState,
}

impl GithubConsistencyChecker {
    async fn check_user_on_github(&self, github_client: &octorust::Client, user: &str) -> Result<(), ConsistencyIssue> {
        github_client
            .users()
            .get_by_username(user)
            .await
            .map_err(|incoming| {
                log::error!("{}", incoming);
                let user_handle = GithubIdentityHandle::new(user.to_owned());
                let ClientError::HttpError { status, .. } = incoming else {
                    return ConsistencyIssue::CannotVerifyUser(user_handle);
                };

                match status {
                    StatusCode::NOT_FOUND => ConsistencyIssue::UserDoesNotExist(user_handle),
                    _ => ConsistencyIssue::CannotVerifyUser(user_handle),
                }
            })
            .map(|_| ())
    }

    async fn check_team_on_github(
        &self,
        github_client: &octorust::Client,
        organization: &str,
        team: &str,
    ) -> Result<(), ConsistencyIssue> {
        github_client
            .teams()
            .get_by_name(organization, team)
            .await
            .map_err(|incoming| {
                log::error!("{}", incoming);
                let org_handle = GithubIdentityHandle::new(organization.to_owned());
                let team_handle = GithubTeamHandle::new(org_handle, team.to_owned());
                let ClientError::HttpError { status, .. } = incoming else {
                    return ConsistencyIssue::CannotVerifyTeam(team_handle);
                };

                match status {
                    StatusCode::NOT_FOUND => ConsistencyIssue::TeamDoesNotExistWithinOrganization(team_handle),
                    _ => ConsistencyIssue::CannotVerifyTeam(team_handle),
                }
            })
            .map(|_| ())
    }

    #[cfg(test)]
    fn check_registered_fake_user(&self, state: &FakeGithubState, username: &str) -> Result<(), ConsistencyIssue> {
        if state.known_users.contains(&username.to_string()) {
            return Ok(());
        };

        let handle = GithubIdentityHandle::new(username.to_owned());
        Err(ConsistencyIssue::UserDoesNotBelongToOrganization(handle))
    }

    #[cfg(test)]
    fn check_registered_fake_team(
        &self,
        state: &FakeGithubState,
        org_name: &str,
        team_name: &str,
    ) -> Result<(), ConsistencyIssue> {
        let formatted = format!("{}/{}", org_name, team_name);
        if state.known_teams.contains(&formatted) {
            return Ok(());
        };

        let org_handle = GithubIdentityHandle::new(org_name.to_owned());
        let handle = GithubTeamHandle::new(org_handle, team_name.to_owned());

        Err(ConsistencyIssue::TeamDoesNotExistWithinOrganization(handle))
    }
}

impl CheckGithubConsistency for GithubConsistencyChecker {
    async fn github_identity(&self, identity: &GithubIdentityHandle) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased(github_client) => {
                self.check_user_on_github(github_client, identity.inner()).await
            },
            #[cfg(test)]
            GithubConsistencyChecker::FakeChecks(state) => self.check_registered_fake_user(state, identity.inner()),
            #[cfg(test)]
            GithubConsistencyChecker::ConsistentState => Ok(()),
        }
    }

    async fn github_team(&self, handle: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased(github_client) => {
                self.check_team_on_github(github_client, handle.organization.inner(), handle.name.as_str())
                    .await
            },
            #[cfg(test)]
            GithubConsistencyChecker::FakeChecks(state) => {
                self.check_registered_fake_team(state, handle.organization.inner(), handle.name.as_str())
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

#[cfg(test)]
mod tests {
    use crate::core::errors::ConsistencyIssue;
    use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
    use crate::infra::github::{CheckGithubConsistency, GithubConsistencyChecker};
    use assertor::{EqualityAssertion, ResultAssertion};
    use httpmock::MockServer;
    use octorust::auth::Credentials;
    use octorust::types::UsersGetByUsernameResponseOneOf;
    use reqwest_retry::policies;

    fn create_github_client(base_url: String) -> octorust::Client {
        let base_http_client = reqwest::Client::builder().build().unwrap();
        let no_retries = policies::ExponentialBackoff::builder().build_with_max_retries(0);
        let custom_http_client = reqwest_middleware::ClientBuilder::new(base_http_client)
            .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(no_retries))
            .build();

        let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let github_token = "test-github-pat".to_string();
        let mut github_client =
            octorust::Client::custom(user_agent, Credentials::Token(github_token), custom_http_client);
        github_client.with_host_override(base_url);
        github_client
    }

    fn create_github_user(username: &str) -> octorust::types::PublicUser {
        octorust::types::PublicUser {
            avatar_url: "https://avatars.githubusercontent.com/u/123456789?v=4".to_string(),
            bio: "".to_string(),
            blog: "".to_string(),
            collaborators: 0,
            company: "".to_string(),
            created_at: None,
            disk_usage: 0,
            email: "".to_string(),
            events_url: "".to_string(),
            followers: 0,
            followers_url: "".to_string(),
            following: 0,
            following_url: "".to_string(),
            gists_url: "".to_string(),
            gravatar_id: "".to_string(),
            hireable: false,
            html_url: "".to_string(),
            id: 123456789,
            location: "".to_string(),
            login: username.to_string(),
            name: username.to_string(),
            node_id: "".to_string(),
            organizations_url: "".to_string(),
            owned_private_repos: 0,
            plan: None,
            private_gists: 0,
            public_gists: 0,
            public_repos: 0,
            received_events_url: "".to_string(),
            repos_url: "".to_string(),
            site_admin: false,
            starred_url: "".to_string(),
            subscriptions_url: "".to_string(),
            suspended_at: None,
            total_private_repos: 0,
            twitter_username: "".to_string(),
            type_: "".to_string(),
            updated_at: None,
            url: "".to_string(),
        }
    }

    #[tokio::test]
    async fn should_report_user_found() {
        let mock_server = MockServer::start();

        let user = UsersGetByUsernameResponseOneOf::PublicUser(create_github_user("ubiratansoares"));

        let existing_public_user = mock_server.mock(|when, then| {
            when.method("GET").path("/users/ubiratansoares");

            then.status(200)
                .header("content-type", "application/json; charset=UTF-8")
                .body(serde_json::to_string(&user).unwrap());
        });

        let github_client = create_github_client(mock_server.base_url());
        let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

        let identity = GithubIdentityHandle::new("ubiratansoares".to_string());
        let check = consistency_checker.github_identity(&identity).await;

        existing_public_user.assert();
        assertor::assert_that!(check).is_ok();
    }

    #[tokio::test]
    async fn should_report_user_not_found() {
        let mock_server = MockServer::start();

        let user_not_found = mock_server.mock(|when, then| {
            when.method("GET").path("/users/ubiratansoares");

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body("not found");
        });

        let github_client = create_github_client(mock_server.base_url());
        let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

        let identity = GithubIdentityHandle::new("ubiratansoares".to_string());
        let check = consistency_checker.github_identity(&identity).await;

        let expected = ConsistencyIssue::UserDoesNotExist(identity);

        user_not_found.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_team_not_found() {
        let existing_organization_name = "dotanuki-labs";
        let missing_team_name = "crabbers";

        let mock_server = MockServer::start();

        let team_not_found = mock_server.mock(|when, then| {
            when.method("GET").path(format!(
                "/orgs/{}/teams/{}",
                existing_organization_name, missing_team_name
            ));

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body("not found");
        });

        let github_client = create_github_client(mock_server.base_url());
        let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

        let organization = GithubIdentityHandle::new(existing_organization_name.to_string());
        let team_handle = GithubTeamHandle::new(organization, missing_team_name.to_string());
        let check = consistency_checker.github_team(&team_handle).await;

        let expected = ConsistencyIssue::TeamDoesNotExistWithinOrganization(team_handle);

        team_not_found.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_user_not_verified() {
        let mock_server = MockServer::start();

        let internal_server_error = mock_server.mock(|when, then| {
            when.method("GET").path("/users/ubiratansoares");

            then.status(500)
                .header("content-type", "application/json; charset=UTF-8")
                .body("Angry unicorns on fire!");
        });

        let github_client = create_github_client(mock_server.base_url());
        let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

        let identity = GithubIdentityHandle::new("ubiratansoares".to_string());
        let check = consistency_checker.github_identity(&identity).await;

        let expected = ConsistencyIssue::CannotVerifyUser(identity);

        internal_server_error.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_team_not_verified() {
        let mock_server = MockServer::start();
        let existing_organization_name = "dotanuki-labs";
        let missing_team_name = "crabbers";

        let internal_server_error = mock_server.mock(|when, then| {
            when.method("GET").path(format!(
                "/orgs/{}/teams/{}",
                existing_organization_name, missing_team_name
            ));

            then.status(500)
                .header("content-type", "application/json; charset=UTF-8")
                .body("Angry unicorns on fire!");
        });

        let github_client = create_github_client(mock_server.base_url());
        let consistency_checker = GithubConsistencyChecker::ApiBased(github_client);

        let organization = GithubIdentityHandle::new(existing_organization_name.to_string());
        let team_handle = GithubTeamHandle::new(organization, missing_team_name.to_string());
        let check = consistency_checker.github_team(&team_handle).await;

        let expected = ConsistencyIssue::CannotVerifyTeam(team_handle);

        internal_server_error.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }
}
