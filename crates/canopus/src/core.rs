// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

pub mod errors;
pub mod models;

#[cfg(test)]
mod tests {
    use crate::core::models::{
        CodeOwners, CodeOwnersEntry, EmailHandle, GithubIdentityHandle, GithubTeamHandle, Owner, OwnershipRecord,
    };
    use assertor::{EqualityAssertion, ResultAssertion};
    use globset::Glob;
    use indoc::indoc;
    use std::collections::HashMap;

    #[test]
    fn should_parse_trivial_codeowners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki-labs/rustaceans
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entries = vec![CodeOwnersEntry::try_new_rule(
            0,
            Glob::new("*.rs")?,
            vec![Owner::GithubTeam(GithubTeamHandle::new(
                GithubIdentityHandle::from("dotanuki-labs"),
                "rustaceans".to_string(),
            ))],
        )?];

        let ownerships = HashMap::from([(
            Owner::GithubTeam(GithubTeamHandle::new(
                GithubIdentityHandle::from("dotanuki-labs"),
                "rustaceans".to_string(),
            )),
            vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(entries, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_comments_and_blank_lines() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            # Rules for dotanuki labs

            *.rs    @dotanuki-labs/devs
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entries = vec![
            CodeOwnersEntry::try_new_comment(0, "Rules for dotanuki labs")?,
            CodeOwnersEntry::BlankLine,
            CodeOwnersEntry::try_new_rule(
                2,
                Glob::new("*.rs")?,
                vec![Owner::GithubTeam(GithubTeamHandle::new(
                    GithubIdentityHandle::from("dotanuki-labs"),
                    "devs".to_string(),
                ))],
            )?,
        ];

        let ownerships = HashMap::from([(
            Owner::GithubTeam(GithubTeamHandle::new(
                GithubIdentityHandle::from("dotanuki-labs"),
                "devs".to_string(),
            )),
            vec![OwnershipRecord::new(2, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(entries, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_commented_rule() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki-labs/crabbers   # Enforce global control
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entries = vec![CodeOwnersEntry::try_new_commented_rule(
            0,
            Glob::new("*.rs")?,
            vec![Owner::GithubTeam(GithubTeamHandle::new(
                GithubIdentityHandle::from("dotanuki-labs"),
                "crabbers".to_string(),
            ))],
            "Enforce global control",
        )?];

        let ownerships = HashMap::from([(
            Owner::GithubTeam(GithubTeamHandle::new(
                GithubIdentityHandle::from("dotanuki-labs"),
                "crabbers".to_string(),
            )),
            vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(entries, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_multiple_owners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @ubiratansoares  rust@dotanuki.dev
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entry = CodeOwnersEntry::try_new_rule(
            0,
            Glob::new("*.rs")?,
            vec![
                Owner::GithubUser(GithubIdentityHandle::from("ubiratansoares")),
                Owner::EmailAddress(EmailHandle::from("rust@dotanuki.dev")),
            ],
        )?;

        let ownerships = HashMap::from([
            (
                Owner::GithubUser(GithubIdentityHandle::from("ubiratansoares")),
                vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
            ),
            (
                Owner::EmailAddress(EmailHandle::new("rust@dotanuki.dev".to_string())),
                vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
            ),
        ]);

        let expected = CodeOwners::new(vec![entry], ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);

        Ok(())
    }

    #[test]
    fn should_fail_with_invalid_comment() {
        let codeowners_rules = indoc! {"
            // Not a valid comment
            *.rs    @org/rustaceans   ufs@dotanuki.dev
        "};

        let parsing = CodeOwners::try_from(codeowners_rules);

        assertor::assert_that!(parsing).is_err();
    }

    #[test]
    fn should_fail_with_unparsable_owner() {
        let codeowners_rules = indoc! {"
            *.rs    ufs.dotanuki
        "};

        let parsing = CodeOwners::try_from(codeowners_rules);

        assertor::assert_that!(parsing).is_err();
    }

    #[test]
    fn should_fail_with_invalid_github_handle() {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki--labs
        "};

        let parsing = CodeOwners::try_from(codeowners_rules);

        assertor::assert_that!(parsing).is_err();
    }
}
