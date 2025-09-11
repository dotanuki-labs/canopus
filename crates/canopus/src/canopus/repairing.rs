// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use crate::core::models::codeowners::CodeOwnersContext;
use itertools::Itertools;

pub fn repair_code_owners(
    codeowners_context: &CodeOwnersContext,
    lines_to_repair: Vec<usize>,
    remove_lines: bool,
) -> anyhow::Result<()> {
    let codeowners_lines = codeowners_context.contents.lines().collect_vec();

    // Evaluate lines to remove or patch
    let new_lines = if remove_lines {
        remove_flagged_lines(&lines_to_repair, &codeowners_lines)
    } else {
        patch_flagged_lines(lines_to_repair, codeowners_lines)
    };

    // Create a new CodeOwners using new lines
    // but also add a new line at the end of the file
    let mut new_codeowners = new_lines.join("\n");
    new_codeowners.push('\n');

    std::fs::write(&codeowners_context.codeowners_path, new_codeowners)?;

    Ok(())
}

fn patch_flagged_lines(lines_to_repair: Vec<usize>, codeowners_lines: Vec<&str>) -> Vec<String> {
    codeowners_lines
        .into_iter()
        .enumerate()
        .map(|(line, content)| {
            if lines_to_repair.contains(&line) {
                format!("# {} (preserved by canopus)", content)
            } else {
                content.to_string()
            }
        })
        .collect_vec()
}

fn remove_flagged_lines(lines_to_repair: &[usize], codeowners_lines: &Vec<&str>) -> Vec<String> {
    codeowners_lines
        .iter()
        .enumerate()
        .filter_map(|(line, content)| {
            if !lines_to_repair.contains(&line) {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use crate::canopus::repairing::repair_code_owners;
    use crate::core::models::codeowners::CodeOwnersContext;
    use assertor::{EqualityAssertion, ResultAssertion};
    use indoc::indoc;
    use temp_dir::TempDir;

    #[test]
    fn should_repair_code_owners_by_removing_lines() {
        let codeowners = indoc! {"
            # Global ownership
            *.rs    @dotanuki/crabbers
            *.js    not-a-valid-owner
        "};

        let temp_dir = TempDir::new().expect("Cant create temp dir");

        let codeowners_location = temp_dir.path().join("CODEOWNERS");

        let codeowners_context = CodeOwnersContext {
            project_path: temp_dir.path().to_path_buf(),
            codeowners_path: codeowners_location,
            contents: codeowners.to_string(),
        };

        let remove_lines = true;
        let lines_to_repair = vec![2];
        let repair = repair_code_owners(&codeowners_context, lines_to_repair, remove_lines);

        assertor::assert_that!(repair).is_ok();

        let repaired = std::fs::read_to_string(&codeowners_context.codeowners_path).unwrap();

        let expected_content = indoc! {"
            # Global ownership
            *.rs    @dotanuki/crabbers
         "};

        assertor::assert_that!(repaired).is_equal_to(expected_content.to_string());
    }

    #[test]
    fn should_repair_code_owners_by_commenting_lines() {
        let codeowners = indoc! {"
            *.rs    @dotanuki/crabbers
            *.js    dotanuki/frontend
        "};

        let temp_dir = TempDir::new().expect("Cant create temp dir");

        let codeowners_location = temp_dir.path().join("CODEOWNERS");

        let codeowners_context = CodeOwnersContext {
            project_path: temp_dir.path().to_path_buf(),
            codeowners_path: codeowners_location,
            contents: codeowners.to_string(),
        };

        let remove_lines = false;
        let lines_to_repair = vec![1];
        let repair = repair_code_owners(&codeowners_context, lines_to_repair, remove_lines);

        assertor::assert_that!(repair).is_ok();

        let repaired = std::fs::read_to_string(&codeowners_context.codeowners_path).unwrap();

        let expected_content = indoc! {"
            *.rs    @dotanuki/crabbers
            # *.js    dotanuki/frontend (preserved by canopus)
         "};

        assertor::assert_that!(repaired).is_equal_to(expected_content.to_string());
    }
}
