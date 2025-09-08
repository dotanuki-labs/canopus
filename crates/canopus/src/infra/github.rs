// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::errors::ConsistencyIssue;
use crate::core::errors::ConsistencyIssue::CannotListMembersInTheOrganization;
use crate::core::models::handles::{GithubIdentityHandle, GithubTeamHandle};
use http::StatusCode;
use itertools::Itertools;
use octocrab::Page;

pub trait CheckGithubConsistency {
    async fn github_identity(&self, organization: &str, handle: &GithubIdentityHandle) -> Result<(), ConsistencyIssue>;

    async fn github_team(&self, organization: &str, handle: &GithubTeamHandle) -> Result<(), ConsistencyIssue>;
}

pub enum GithubConsistencyChecker {
    ApiBased(octocrab::Octocrab),
    #[cfg(test)]
    FakeChecks(FakeGithubState),
    #[cfg(test)]
    ConsistentState,
}

impl GithubConsistencyChecker {
    async fn get_github_users_per_page(
        github_client: &octocrab::Octocrab,
        page: u32,
        organization: &str,
    ) -> Result<Vec<GithubIdentityHandle>, ConsistencyIssue> {
        let members = github_client
            .orgs(organization)
            .list_members()
            .page(page)
            .per_page(100)
            .send()
            .await
            .or_else(|error| match error {
                octocrab::Error::GitHub { source, .. } => {
                    if source.status_code == StatusCode::NOT_FOUND {
                        Ok(Page::default())
                    } else {
                        Err(CannotListMembersInTheOrganization(organization.to_string()))
                    }
                },
                _ => Err(CannotListMembersInTheOrganization(organization.to_string())),
            })?;

        let handles = members
            .into_iter()
            .map(|user| GithubIdentityHandle::new(user.login))
            .collect_vec();

        Ok(handles)
    }

    async fn find_all_users_for_organization(
        &self,
        github_client: &octocrab::Octocrab,
        organization: &str,
    ) -> Result<Vec<GithubIdentityHandle>, ConsistencyIssue> {
        let mut all_handles = Vec::new();
        let mut page = 0;

        loop {
            page += 1;
            let handles = Self::get_github_users_per_page(github_client, page, organization).await?;

            if handles.is_empty() {
                break;
            }

            all_handles.extend(handles);
        }

        Ok(all_handles)
    }

    async fn check_user_on_github(
        &self,
        github_client: &octocrab::Octocrab,
        organization: &str,
        user: &str,
    ) -> Result<(), ConsistencyIssue> {
        let users_in_organization = self
            .find_all_users_for_organization(github_client, organization)
            .await?;

        let target_user = GithubIdentityHandle::new(user.to_string());

        let user_listed_in_organization = users_in_organization.contains(&target_user);

        if user_listed_in_organization {
            return Ok(());
        }

        github_client
            .users(user)
            .profile()
            .await
            .map_err(|incoming| {
                println!("{:?}", incoming);
                log::info!("Failed to fetch info for {} user on Github", user);

                let handle = target_user.clone();

                let octocrab::Error::GitHub { source, .. } = incoming else {
                    return ConsistencyIssue::CannotVerifyUser(handle);
                };

                match source.status_code {
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
        github_client: &octocrab::Octocrab,
        organization: &str,
        team: &str,
    ) -> Result<(), ConsistencyIssue> {
        github_client
            .teams(organization)
            .get(team)
            .await
            .map_err(|incoming| {
                log::info!("Failed to fetch info for {}/{} team on Github", organization, team);

                let org_handle = GithubIdentityHandle::new(organization.to_owned());
                let team_handle = GithubTeamHandle::new(org_handle, team.to_owned());

                let octocrab::Error::GitHub { source, .. } = incoming else {
                    return ConsistencyIssue::CannotVerifyTeam(team_handle);
                };

                match source.status_code {
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
    use http::Uri;
    use httpmock::{MockServer, Then, When};
    use itertools::Itertools;
    use octocrab::service::middleware::retry::RetryConfig;
    use std::str::FromStr;

    struct ServerUriFactory(String);

    impl TryInto<Uri> for ServerUriFactory {
        type Error = http::uri::InvalidUri;

        fn try_into(self) -> Result<Uri, Self::Error> {
            Uri::from_str(self.0.as_str())
        }
    }

    fn create_github_client(base_url: String) -> octocrab::Octocrab {
        octocrab::Octocrab::builder()
            .base_uri(ServerUriFactory(base_url))
            .unwrap()
            .add_retry_config(RetryConfig::Simple(0))
            .build()
            .unwrap()
    }

    fn responds_with_existing_github_user(username: &str) -> impl FnOnce(When, Then) {
        move |when, then| {
            let user_template = r#"{
                  "login": "<username>",
                  "id": 1,
                  "node_id": "MDQ6VXNlcjE=",
                  "avatar_url": "https://github.com/images/<username>.jpg",
                  "gravatar_id": "abcdedf",
                  "url": "https://api.github.com/users/<username>",
                  "html_url": "https://github.com/<username>",
                  "followers_url": "https://api.github.com/users/<username>/followers",
                  "following_url": "https://api.github.com/users/<username>/following",
                  "gists_url": "https://api.github.com/users/<username>/gists",
                  "starred_url": "https://api.github.com/users/<username>/starred",
                  "subscriptions_url": "https://api.github.com/users/<username>/subscriptions",
                  "organizations_url": "https://api.github.com/users/<username>/orgs",
                  "repos_url": "https://api.github.com/users/<username>/repos",
                  "events_url": "https://api.github.com/users/<username>/events",
                  "received_events_url": "https://api.github.com/users/<username>/received_events",
                  "type": "User",
                  "site_admin": false,
                  "name": "<username>",
                  "company": "ACME",
                  "blog": "https://github.com/blog",
                  "hireable": false,
                  "public_repos": 0,
                  "public_gists": 0,
                  "followers": 0,
                  "following": 0,
                  "created_at": "2025-02-10T04:33:00Z",
                  "updated_at": "2025-03-20T06:55:00Z"
                }"#;

            let user = user_template.replace("<username>", username);

            println!("{user}");

            when.method("GET").path(format!("/users/{}", username));

            then.status(200)
                .header("content-type", "application/json; charset=UTF-8")
                .body(user);
        }
    }

    fn responds_with_user_not_found_on_github(username: &str) -> impl FnOnce(When, Then) {
        let not_found = r#"{
            "message" : "not found"
        }"#;

        move |when, then| {
            when.method("GET").path(format!("/users/{}", username));

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body(not_found);
        }
    }

    fn responds_with_team_not_found(organization: &str, team_name: &str) -> impl FnOnce(When, Then) {
        let not_found = r#"{
            "message" : "not found"
        }"#;

        move |when, then| {
            when.method("GET")
                .path(format!("/orgs/{}/teams/{}", organization, team_name));

            then.status(404)
                .header("content-type", "application/json; charset=UTF-8")
                .body(not_found);
        }
    }

    fn responds_with_internal_error(api_path: &str) -> impl FnOnce(When, Then) {
        let server_crash = r#"{
            "message" : "unicorns are angry right now"
        }"#;

        move |when, then| {
            when.method("GET").path(api_path);

            then.status(500)
                .header("content-type", "application/json; charset=UTF-8")
                .body(server_crash);
        }
    }

    fn responds_with_members_of_an_organization(organization: &str, usernames: Vec<&str>) -> impl FnOnce(When, Then) {
        let member_template = r#"
                  {
                    "login": "<username>",
                    "id": 0,
                    "node_id": "<username>",
                    "avatar_url": "https://github.com/images/<username>.jpeg",
                    "gravatar_id": "https://gravatar.com/images/<username>.jpeg",
                    "url": "https://api.github.com/users/<username>",
                    "html_url": "https://github.com/<username>",
                    "followers_url": "https://api.github.com/users/<username>/followers",
                    "following_url": "https://api.github.com/users/<username>/following",
                    "gists_url": "https://api.github.com/users/<username>/gists",
                    "starred_url": "https://api.github.com/users/<username>/starred",
                    "subscriptions_url": "https://api.github.com/users/<username>/subscriptions",
                    "organizations_url": "https://api.github.com/users/<username>/orgs",
                    "repos_url": "https://api.github.com/users/<username>/repos",
                    "events_url": "https://api.github.com/users/<username>/events",
                    "received_events_url": "https://api.github.com/users/<username>/received_events",
                    "type": "User",
                    "site_admin": false
                  }
            "#;

        move |when, then| {
            let users = usernames
                .into_iter()
                .map(|username| member_template.replace("<username>", username))
                .collect_vec()
                .join(",");

            let json = format!("[{}]", users);

            when.method("GET")
                .path(format!("/orgs/{}/members", organization))
                .query_param("page", "1")
                .query_param("per_page", "100");

            then.status(200)
                .header("content-type", "application/json; charset=UTF-8")
                .body(json);
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

        let expected = ConsistencyIssue::CannotListMembersInTheOrganization("dotanuki".to_string());

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
