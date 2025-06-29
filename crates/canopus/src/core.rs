// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

pub mod errors;
pub mod models;

#[cfg(test)]
mod tests {
    use crate::core::models::{CodeOwners, CodeOwnersEntry, Owner};
    use assertor::{EqualityAssertion, ResultAssertion};
    use globset::Glob;
    use indoc::indoc;

    #[test]
    fn should_parse_trivial_codeowners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @org/rustaceans
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let expected = CodeOwners {
            entries: vec![CodeOwnersEntry::try_new_rule(
                0,
                Glob::new("*.rs")?,
                vec![Owner::try_from((0, "@org/rustaceans"))?],
            )?],
        };

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_comments_and_blank_lines() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            # Rules for dotanuki labs

            *.rs    @org/rustaceans
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let expected = CodeOwners {
            entries: vec![
                CodeOwnersEntry::try_new_comment(0, "Rules for dotanuki labs")?,
                CodeOwnersEntry::BlankLine,
                CodeOwnersEntry::try_new_rule(
                    2,
                    Glob::new("*.rs")?,
                    vec![Owner::GithubTeam("org/rustaceans".to_string())],
                )?,
            ],
        };

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_commented_rule() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @org/rustaceans   # Enforce global control
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules).unwrap();

        let expected = CodeOwners {
            entries: vec![CodeOwnersEntry::try_new_commented_rule(
                0,
                Glob::new("*.rs")?,
                vec![Owner::GithubTeam("org/rustaceans".to_string())],
                "Enforce global control",
            )?],
        };

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_multiple_owners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @ubiratansoares  rust@dotanuki.dev
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let expected = CodeOwners {
            entries: vec![CodeOwnersEntry::try_new_rule(
                0,
                Glob::new("*.rs")?,
                vec![
                    Owner::GithubUser("ubiratansoares".to_string()),
                    Owner::EmailAddress("rust@dotanuki.dev".to_string()),
                ],
            )?],
        };

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
    fn should_fail_with_invalid_owner() {
        let codeowners_rules = indoc! {"
            *.rs    ufs.dotanuki
        "};

        let parsing = CodeOwners::try_from(codeowners_rules);

        assertor::assert_that!(parsing).is_err();
    }
}
