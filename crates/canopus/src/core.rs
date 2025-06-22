// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

#[derive(Clone, Debug, PartialEq)]
pub enum Owner {
    GithubUser(String),
    GithubTeam(String),
    EmailAddress(String),
}

impl TryFrom<&str> for Owner {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> anyhow::Result<Self> {
        if value.starts_with("@") {
            let normalized = value.replacen("@", "", 1);
            if value.contains("/") {
                return Ok(Owner::GithubTeam(normalized));
            } else {
                return Ok(Owner::GithubUser(normalized));
            }
        };

        if email_address::EmailAddress::is_valid(value) {
            return Ok(Owner::EmailAddress(value.to_string()));
        };

        Err(anyhow::anyhow!("Invalid owner: {}", value))
    }
}

#[derive(Debug, PartialEq)]
pub struct Ownership {
    pub pattern: String,
    pub owners: Vec<Owner>,
    pub inline_comment: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum CodeOwnersEntry {
    BlankLine,
    Comment(String),
    Rule(Ownership),
}

impl CodeOwnersEntry {
    fn new_comment(comment: &str) -> Self {
        CodeOwnersEntry::Comment(comment.replace("#", "").trim().to_string())
    }

    fn new_rule(pattern: &str, owners: &[Owner]) -> Self {
        CodeOwnersEntry::Rule(Ownership {
            pattern: pattern.to_string(),
            owners: owners.to_vec(),
            inline_comment: None,
        })
    }

    fn new_commented_rule(pattern: &str, owners: &[Owner], comment: &str) -> Self {
        CodeOwnersEntry::Rule(Ownership {
            pattern: pattern.to_string(),
            owners: owners.to_vec(),
            inline_comment: Some(comment.to_string()),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeOwners {
    pub entries: Vec<CodeOwnersEntry>,
}

impl TryFrom<(usize, &str)> for CodeOwnersEntry {
    type Error = anyhow::Error;

    fn try_from((line_number, line_contents): (usize, &str)) -> anyhow::Result<Self> {
        if line_contents.is_empty() {
            Ok(CodeOwnersEntry::BlankLine)
        } else if line_contents.starts_with("#") {
            Ok(CodeOwnersEntry::new_comment(line_contents))
        } else {
            let mut parts = line_contents.split_whitespace();

            let Some(pattern) = parts.next() else {
                return Err(anyhow::anyhow!("Invalid line: {}", line_number));
            };

            let mut owners: Vec<Owner> = vec![];
            let mut inline_comments: Vec<&str> = vec![];
            let mut inline_comment_detected = false;

            for item in parts {
                if item == "#" {
                    inline_comment_detected = true;
                    continue;
                }

                if inline_comment_detected {
                    inline_comments.push(item);
                } else {
                    owners.push(Owner::try_from(item)?);
                }
            }

            if owners.is_empty() {
                return Err(anyhow::anyhow!("No owners detected: {}", line_number));
            }

            if inline_comment_detected {
                let inline_comment = inline_comments.join(" ");
                return Ok(CodeOwnersEntry::new_commented_rule(pattern, &owners, &inline_comment));
            } else {
                return Ok(CodeOwnersEntry::new_rule(pattern, &owners));
            };
        }
    }
}

impl TryFrom<&str> for CodeOwners {
    type Error = anyhow::Error;

    fn try_from(content: &str) -> anyhow::Result<Self> {
        let lines = content.lines();

        let mut entries: Vec<CodeOwnersEntry> = vec![];

        for (line_number, line_contents) in lines.enumerate() {
            entries.push(CodeOwnersEntry::try_from((line_number, line_contents))?);
        }

        Ok(CodeOwners { entries })
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{CodeOwners, CodeOwnersEntry, Owner};
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;

    #[test]
    fn should_parse_trivial_codeowners() -> anyhow::Result<()> {
        let codeowners_rules = indoc! {"
            *.rs    @org/rustaceans
        "};

        let codeowners = CodeOwners::try_from(codeowners_rules)?;

        let expected = CodeOwners {
            entries: vec![CodeOwnersEntry::new_rule(
                "*.rs",
                &[Owner::try_from("@org/rustaceans")?],
            )],
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
                CodeOwnersEntry::new_comment("Rules for dotanuki labs"),
                CodeOwnersEntry::BlankLine,
                CodeOwnersEntry::new_rule("*.rs", &[Owner::GithubTeam("org/rustaceans".to_string())]),
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
            entries: vec![CodeOwnersEntry::new_commented_rule(
                "*.rs",
                &[Owner::GithubTeam("org/rustaceans".to_string())],
                "Enforce global control",
            )],
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
            entries: vec![CodeOwnersEntry::new_rule(
                "*.rs",
                &[
                    Owner::GithubUser("ubiratansoares".to_string()),
                    Owner::EmailAddress("rust@dotanuki.dev".to_string()),
                ],
            )],
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
