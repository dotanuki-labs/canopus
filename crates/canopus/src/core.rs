// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

pub mod models;

#[cfg(test)]
mod tests {
    use crate::core::models::codeowners::{CodeOwners, CodeOwnersEntry, OwnershipRecord};
    use crate::core::models::handles::Owner;
    use crate::core::models::test_helpers::ValidationIssueKindFactory;
    use crate::core::models::{ValidationIssue, ValidationOutcome};
    use assertor::EqualityAssertion;
    use globset::Glob;
    use indoc::indoc;
    use std::collections::HashMap;

    #[test]
    fn should_parse_trivial_codeowners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki-labs/rustaceans
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entry = CodeOwnersEntry::ownership(0, "*.rs", "@dotanuki-labs/rustaceans");

        let ownerships = HashMap::from([(
            Owner::from("@dotanuki-labs/rustaceans"),
            vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(vec![entry], ValidationOutcome::NoIssues, ownerships);

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
            CodeOwnersEntry::comment(0, "# Rules for dotanuki labs"),
            CodeOwnersEntry::BlankLine,
            CodeOwnersEntry::ownership(2, "*.rs", "@dotanuki-labs/devs"),
        ];

        let ownerships = HashMap::from([(
            Owner::from("@dotanuki-labs/devs"),
            vec![OwnershipRecord::new(2, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(entries, ValidationOutcome::NoIssues, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_commented_rule() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki-labs/crabbers   # Enforce global control
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entry =
            CodeOwnersEntry::detailed_ownership(0, "*.rs", "@dotanuki-labs/crabbers", "# Enforce global control");

        let ownerships = HashMap::from([(
            Owner::from("@dotanuki-labs/crabbers"),
            vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
        )]);

        let expected = CodeOwners::new(vec![entry], ValidationOutcome::NoIssues, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);
        Ok(())
    }

    #[test]
    fn should_parse_multiple_owners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @ubiratansoares  rust@dotanuki.dev
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let entry = CodeOwnersEntry::ownership(0, "*.rs", "@ubiratansoares rust@dotanuki.dev");

        let ownerships = HashMap::from([
            (
                Owner::from("@ubiratansoares"),
                vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
            ),
            (
                Owner::from("rust@dotanuki.dev"),
                vec![OwnershipRecord::new(0, Glob::new("*.rs")?)],
            ),
        ]);

        let expected = CodeOwners::new(vec![entry], ValidationOutcome::NoIssues, ownerships);

        assertor::assert_that!(codeowners).is_equal_to(expected);

        Ok(())
    }

    #[test]
    fn should_accept_empty_comment() {
        let codeowners_rules = indoc! {"
            #
            *.rs    @org/rustaceans   ufs@dotanuki.dev
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules).unwrap();

        let expected = ValidationOutcome::NoIssues;

        assertor::assert_that!(codeowners.syntax_validation).is_equal_to(expected);
    }

    #[test]
    fn should_fail_with_unparsable_owner() {
        let codeowners_rules = indoc! {"
            *.rs    ufs.dotanuki
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules).unwrap();

        let syntax_issues = vec![
            ValidationIssue::builder()
                .kind(ValidationIssueKindFactory::invalid_syntax())
                .line_number(0)
                .description("cannot parse owner")
                .build(),
        ];

        let expected = ValidationOutcome::IssuesDetected(syntax_issues);

        assertor::assert_that!(codeowners.syntax_validation).is_equal_to(expected);
    }

    #[test]
    fn should_fail_with_invalid_github_handle() {
        let codeowners_rules = indoc! {"
            *.rs    @dotanuki--labs
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules).unwrap();

        let syntax_issues = vec![
            ValidationIssue::builder()
                .kind(ValidationIssueKindFactory::invalid_syntax())
                .line_number(0)
                .description("cannot parse owner")
                .build(),
        ];

        let expected = ValidationOutcome::IssuesDetected(syntax_issues);

        assertor::assert_that!(codeowners.syntax_validation).is_equal_to(expected);
    }
}
