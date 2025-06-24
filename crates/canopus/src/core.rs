// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use ValidationError::{
    CannotParseComment, CannotParseGlobPattern, CannotParseOwner, DanglingGlobPatterns, NoOwnersDetected,
};
use anyhow::bail;
use globset::Glob;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct DanglingGlobPattern {
    pub line_number: usize,
    pub pattern: String,
}

#[derive(Debug)]
pub enum ValidationError {
    CannotParseComment { line: usize },
    CannotParseGlobPattern { line: usize },
    CannotParseOwner { line: usize },
    NoOwnersDetected { line: usize },
    DanglingGlobPatterns { patterns: Vec<DanglingGlobPattern> },
}

impl Display for ValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            CannotParseComment { line } => format!("L{} : cannot parse comment", line),
            CannotParseGlobPattern { line } => format!("L{} : cannot parse glob pattern", line),
            CannotParseOwner { line } => format!("L{} : cannot parse owner", line),
            NoOwnersDetected { line } => format!("L{} : no owners found", line),
            DanglingGlobPatterns { patterns } => patterns
                .iter()
                .map(|dangling| {
                    format!(
                        "L{} : pattern {} does not match any project path",
                        dangling.line_number, dangling.pattern
                    )
                })
                .collect::<Vec<String>>()
                .join("\n"),
        };

        f.write_str(&message)
    }
}

impl std::error::Error for ValidationError {
    // Already satisfied
}

#[derive(Clone, Debug, PartialEq)]
pub enum Owner {
    GithubUser(String),
    GithubTeam(String),
    EmailAddress(String),
}

impl TryFrom<(usize, &str)> for Owner {
    type Error = anyhow::Error;

    fn try_from((line, value): (usize, &str)) -> anyhow::Result<Self> {
        if value.starts_with("@") {
            let normalized = value.replacen("@", "", 1);
            return if value.contains("/") {
                Ok(Owner::GithubTeam(normalized))
            } else {
                Ok(Owner::GithubUser(normalized))
            };
        };

        if email_address::EmailAddress::is_valid(value) {
            return Ok(Owner::EmailAddress(value.to_string()));
        };

        bail!(CannotParseOwner { line });
    }
}

#[derive(Debug, PartialEq)]
pub struct Ownership {
    pub line_number: usize,
    pub glob: Glob,
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
    fn try_new_comment(line_number: usize, comment: &str) -> anyhow::Result<Self> {
        if comment.is_empty() {
            bail!(CannotParseComment { line: line_number });
        };

        let sanitized = comment.replace("#", "").trim().to_string();
        Ok(CodeOwnersEntry::Comment(sanitized))
    }

    fn try_new_rule(line_number: usize, glob: Glob, owners: Vec<Owner>) -> anyhow::Result<Self> {
        if owners.is_empty() {
            bail!(NoOwnersDetected { line: line_number });
        }

        let ownership = Ownership {
            line_number,
            glob,
            owners,
            inline_comment: None,
        };

        Ok(CodeOwnersEntry::Rule(ownership))
    }

    fn try_new_commented_rule(
        line_number: usize,
        glob: Glob,
        owners: Vec<Owner>,
        comment: &str,
    ) -> anyhow::Result<Self> {
        if comment.is_empty() {
            bail!(CannotParseComment { line: line_number });
        };

        if owners.is_empty() {
            bail!(NoOwnersDetected { line: line_number });
        }

        let ownership = Ownership {
            line_number,
            glob,
            owners,
            inline_comment: Some(comment.to_string()),
        };

        Ok(CodeOwnersEntry::Rule(ownership))
    }
}

impl TryFrom<(usize, &str)> for CodeOwnersEntry {
    type Error = anyhow::Error;

    fn try_from((line_number, line_contents): (usize, &str)) -> anyhow::Result<Self> {
        if line_contents.is_empty() {
            Ok(CodeOwnersEntry::BlankLine)
        } else if line_contents.starts_with("#") {
            CodeOwnersEntry::try_new_comment(line_number, line_contents)
        } else {
            let mut parts = line_contents.split_whitespace();

            let Some(raw_pattern) = parts.next() else {
                panic!("L{} : expecting non-empty line", line_number)
            };

            let glob = Glob::new(raw_pattern).map_err(|_| CannotParseGlobPattern { line: line_number })?;
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
                    owners.push(Owner::try_from((line_number, item))?);
                }
            }

            return if inline_comment_detected {
                let inline_comment = inline_comments.join(" ");
                CodeOwnersEntry::try_new_commented_rule(line_number, glob, owners, &inline_comment)
            } else {
                CodeOwnersEntry::try_new_rule(line_number, glob, owners)
            };
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct CodeOwners {
    pub entries: Vec<CodeOwnersEntry>,
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
