// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::ConsistencyIssue;
use crate::core::errors::ConsistencyIssue::CannotListMembersInTheOrganization;
use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
use futures::TryFutureExt;
use itertools::Itertools;
use octorust::{ClientError, StatusCode, types};

pub trait CheckGithubConsistency {
    async fn github_identity(&self, organization: &str, handle: &GithubIdentityHandle) -> Result<(), ConsistencyIssue>;

    async fn github_team(&self, organization: &str, handle: &GithubTeamHandle) -> Result<(), ConsistencyIssue>;
}

pub enum GithubConsistencyChecker {
    ApiBased(octorust::Client),
    #[cfg(test)]
    FakeChecks(FakeGithubState),
    #[cfg(test)]
    ConsistentState,
}

impl GithubConsistencyChecker {
    async fn find_users_for_organization(
        &self,
        github_client: &octorust::Client,
        organization: &str,
    ) -> Result<Vec<GithubIdentityHandle>, ConsistencyIssue> {
        let filter = types::OrgsListMembersFilter::All;
        let role = types::OrgsListMembersRole::All;

        let all_members = github_client
            .orgs()
            .list_all_members(organization, filter, role)
            .map_err(|_| CannotListMembersInTheOrganization)
            .await?;

        let all_handles = all_members
            .body
            .into_iter()
            .map(|user| GithubIdentityHandle::new(user.login))
            .collect_vec();

        Ok(all_handles)
    }

    async fn check_user_on_github(
        &self,
        github_client: &octorust::Client,
        organization: &str,
        user: &str,
    ) -> Result<(), ConsistencyIssue> {
        let users_in_organization = self.find_users_for_organization(github_client, organization).await?;

        let target_user = GithubIdentityHandle::new(user.to_string());

        let user_listed_in_organization = users_in_organization.contains(&target_user);

        if user_listed_in_organization {
            return Ok(());
        }

        github_client
            .users()
            .get_by_username(user)
            .await
            .map_err(|incoming| {
                log::error!("{}", incoming);

                let handle = target_user.clone();

                let ClientError::HttpError { status, .. } = incoming else {
                    return ConsistencyIssue::CannotVerifyUser(handle);
                };

                match status {
                    StatusCode::NOT_FOUND => ConsistencyIssue::UserDoesNotExist(handle),
                    _ => ConsistencyIssue::CannotVerifyUser(handle),
                }
            })
            .map(|_| ())?;

        if !user_listed_in_organization {
            return Err(ConsistencyIssue::UserDoesNotBelongToOrganization(target_user));
        };

        Ok(())
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
    async fn github_identity(
        &self,
        organization: &str,
        identity: &GithubIdentityHandle,
    ) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased(github_client) => {
                self.check_user_on_github(github_client, organization, identity.inner())
                    .await
            },
            #[cfg(test)]
            GithubConsistencyChecker::FakeChecks(state) => self.check_registered_fake_user(state, identity.inner()),
            #[cfg(test)]
            GithubConsistencyChecker::ConsistentState => Ok(()),
        }
    }

    async fn github_team(&self, organization: &str, handle: &GithubTeamHandle) -> Result<(), ConsistencyIssue> {
        match self {
            GithubConsistencyChecker::ApiBased(github_client) => {
                let defined_organization = handle.organization.inner();
                if defined_organization != organization {
                    return Err(ConsistencyIssue::TeamDoesNotMatchWithProvidedOrganization(
                        handle.clone(),
                    ));
                };

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
    use httpmock::{MockServer, Then, When};
    use itertools::Itertools;
    use octorust::auth::Credentials;
    use rand::random;
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

    fn responds_with_existing_github_user(username: &str) -> impl FnOnce(When, Then) {
        move |when, then| {
            let user_id = random::<u64>();
            let user = octorust::types::PublicUser {
                avatar_url: format!("https://avatars.githubusercontent.com/u/{}?v=4", user_id),
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
                id: user_id as i64,
                location: "".to_string(),
                login: username.to_string(),
                name: username.to_string(),
                node_id: random::<u64>().to_string(),
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
            };

            let response = serde_json::to_string(&user).unwrap();

            when.method("GET").path(format!("/users/{}", username));

            then.status(200)
                .header("content-type", "application/json; charset=UTF-8")
                .body(response);
        }
    }

    fn responds_with_user_not_found_on_github(username: &str) -> impl FnOnce(When, Then) {
        move |when, then| {
            when.method("GET").path(format!("/users/{}", username));

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body("not found");
        }
    }

    fn responds_with_team_not_found(organization: &str, team_name: &str) -> impl FnOnce(When, Then) {
        move |when, then| {
            when.method("GET")
                .path(format!("/orgs/{}/teams/{}", organization, team_name));

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body("not found");
        }
    }

    fn responds_with_internal_error(api_path: &str) -> impl FnOnce(When, Then) {
        move |when, then| {
            when.method("GET").path(api_path);

            then.status(500)
                .header("content-type", "application/json; charset=UTF-8")
                .body("Angry unicorn");
        }
    }

    fn responds_with_members_of_an_organization(organization: &str, usernames: Vec<&str>) -> impl FnOnce(When, Then) {
        move |when, then| {
            let users = usernames
                .into_iter()
                .map(|username| {
                    let user_id = random::<u64>();

                    octorust::types::SimpleUser {
                        avatar_url: format!("https://avatars.githubusercontent.com/u/{}?v=4", user_id),
                        email: "".to_string(),
                        events_url: "".to_string(),
                        followers_url: "".to_string(),
                        following_url: "".to_string(),
                        gists_url: "".to_string(),
                        gravatar_id: "".to_string(),
                        html_url: "".to_string(),
                        id: user_id as i64,
                        login: username.to_string(),
                        name: "".to_string(),
                        node_id: "".to_string(),
                        organizations_url: format!("https://github.com/{}", organization),
                        received_events_url: "".to_string(),
                        repos_url: "".to_string(),
                        site_admin: false,
                        starred_at: "".to_string(),
                        starred_url: "".to_string(),
                        subscriptions_url: "".to_string(),
                        type_: "".to_string(),
                        url: "".to_string(),
                    }
                })
                .collect_vec();

            when.method("GET").path(format!("/orgs/{}/members", organization));

            then.status(200)
                .header("content-type", "application/json; charset=UTF-8")
                .body(serde_json::to_string(&users).unwrap());
        }
    }

    #[tokio::test]
    async fn should_report_user_found_within_organization_members() {
        let mock_server = MockServer::start();

        let github_organization = "dotanuki-labs";
        let members = vec!["ubiratansoares", "dotanuki-bot"];

        let returns_members = responds_with_members_of_an_organization(github_organization, members);

        let organization_members = mock_server.mock(returns_members);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let identity = GithubIdentityHandle::new("ubiratansoares".to_string());
        let check = consistency_checker
            .github_identity(github_organization, &identity)
            .await;

        organization_members.assert();
        assertor::assert_that!(check).is_ok();
    }

    #[tokio::test]
    async fn should_report_user_outside_github_organization() {
        let mock_server = MockServer::start();

        let github_organization = "dotanuki-labs";
        let members = vec!["ubiratansoares", "dotanuki-bot"];
        let outside_organization = "itto-ogami";

        let returns_members = responds_with_members_of_an_organization(github_organization, members);

        let returns_user_on_github = responds_with_existing_github_user(outside_organization);

        let organization_members = mock_server.mock(returns_members);
        let exists_on_github = mock_server.mock(returns_user_on_github);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let identity = GithubIdentityHandle::new(outside_organization.to_string());
        let check = consistency_checker
            .github_identity(github_organization, &identity)
            .await;

        let expected = ConsistencyIssue::UserDoesNotBelongToOrganization(identity);

        organization_members.assert();
        exists_on_github.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_user_not_found() {
        let mock_server = MockServer::start();

        let github_organization = "dotanuki-labs";
        let members = vec!["ubiratansoares", "dotanuki-bot"];
        let not_on_github = "itto-ogami";

        let returns_members = responds_with_members_of_an_organization(github_organization, members);

        let returns_user_not_found = responds_with_user_not_found_on_github(not_on_github);

        let organization_members = mock_server.mock(returns_members);
        let user_not_found = mock_server.mock(returns_user_not_found);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let identity = GithubIdentityHandle::new(not_on_github.to_string());
        let check = consistency_checker
            .github_identity(github_organization, &identity)
            .await;

        let expected = ConsistencyIssue::UserDoesNotExist(identity);

        organization_members.assert();
        user_not_found.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_team_does_not_match() {
        let provided_github_organization = "dotanuki-labs";
        let misspelled_organization = "dotanuki";
        let github_team = "crabbers";

        let consistency_checker =
            GithubConsistencyChecker::ApiBased(create_github_client("https://api.github.com".to_string()));

        let organization = GithubIdentityHandle::new(misspelled_organization.to_string());
        let team_handle = GithubTeamHandle::new(organization, github_team.to_string());
        let check = consistency_checker
            .github_team(provided_github_organization, &team_handle)
            .await;

        let expected = ConsistencyIssue::TeamDoesNotMatchWithProvidedOrganization(team_handle);

        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_team_not_found() {
        let mock_server = MockServer::start();

        let github_organization = "dotanuki-labs";
        let undefined_team = "crabbers";

        let returns_not_found = responds_with_team_not_found(github_organization, undefined_team);

        let team_not_found = mock_server.mock(returns_not_found);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let organization = GithubIdentityHandle::new(github_organization.to_string());
        let team_handle = GithubTeamHandle::new(organization, undefined_team.to_string());
        let check = consistency_checker.github_team(github_organization, &team_handle).await;

        let expected = ConsistencyIssue::TeamDoesNotExistWithinOrganization(team_handle);

        team_not_found.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_user_not_verified() {
        let mock_server = MockServer::start();

        let returns_internal_error = responds_with_internal_error("/orgs/dotanuki/members");
        let internal_server_error = mock_server.mock(returns_internal_error);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let identity = GithubIdentityHandle::new("ubiratansoares".to_string());
        let check = consistency_checker.github_identity("dotanuki", &identity).await;

        let expected = ConsistencyIssue::CannotListMembersInTheOrganization;

        internal_server_error.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }

    #[tokio::test]
    async fn should_report_team_not_verified() {
        let mock_server = MockServer::start();

        let returns_internal_error = responds_with_internal_error("/orgs/dotanuki/teams/crabbers");
        let internal_server_error = mock_server.mock(returns_internal_error);

        let consistency_checker = GithubConsistencyChecker::ApiBased(create_github_client(mock_server.base_url()));

        let organization = GithubIdentityHandle::new("dotanuki".to_string());
        let team_handle = GithubTeamHandle::new(organization, "crabbers".to_string());
        let check = consistency_checker.github_team("dotanuki", &team_handle).await;

        let expected = ConsistencyIssue::CannotVerifyTeam(team_handle);

        internal_server_error.assert();
        assertor::assert_that!(check).is_equal_to(Err(expected));
    }
}
