// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use assert_cmd::Command;
use assert_cmd::assert::Assert;
use indoc::indoc;
use predicates::str::contains;
use std::fs;
use temp_dir::TempDir;

fn sut() -> Command {
    let _ = env_logger::builder().is_test(true).try_init();
    Command::cargo_bin("canopus").expect("Failed to create a command")
}

fn validate_codeowners(contents: &str) -> Assert {
    let temp_dir = TempDir::new().expect("Cant create temp dir");
    let target = temp_dir.path().join("CODEOWNERS");
    fs::write(&target, contents).expect("Failed to write content to CODEOWNERS file");
    let project_path = target.parent().unwrap().to_str().unwrap();

    sut().args(["validate", "-p", project_path]).assert()
}

#[test]
fn should_detect_no_codeowners() {
    let temp_dir = TempDir::new().expect("Cant create temp dir");

    let project_path = temp_dir.path().to_str().unwrap();
    let execution = sut().args(["validate", "-p", project_path]).assert();

    execution
        .failure()
        .stderr(contains("no CODEOWNERS definition found in the project"));
}

#[test]
fn should_detect_multiple_codeowners() {
    let codeowners = indoc! {"
        # Basic syntax
        *.rs    @org/crabbers
    "};

    let temp_dir = TempDir::new().expect("Cant create temp dir");

    let some_config = temp_dir.path().join("CODEOWNERS");
    fs::write(&some_config, codeowners).expect("failed to write content to CODEOWNERS file");

    fs::create_dir_all(temp_dir.child(".github")).expect("Failed to create .github dir");

    let another_config = temp_dir.path().join(".github/CODEOWNERS");
    fs::write(&another_config, codeowners).expect("failed to write content to CODEOWNERS file");

    let project_path = some_config.parent().unwrap().to_str().unwrap();
    let execution = sut().args(["validate", "-p", project_path]).assert();

    execution
        .failure()
        .stderr(contains("found multiple definitions for CODEOWNERS"));
}

#[test]
fn should_detect_single_codeowners_file() {
    let codeowners = indoc! {"
        *.rs    @org/crabbers
    "};

    let execution = validate_codeowners(codeowners);

    execution
        .failure()
        .stderr(contains("pattern *.rs does not match any project path"));
}

#[test]
fn should_detect_glob_syntax_issue() {
    let codeowners = indoc! {"
        [z-a]*.rs    @org/crabbers
    "};

    let execution = validate_codeowners(codeowners);

    execution.failure().stderr(contains("cannot parse glob pattern"));
}

#[test]
fn should_detect_owner_syntax_issue() {
    let codeowners = indoc! {"
        *.rs    org/crabbers
    "};

    let execution = validate_codeowners(codeowners);

    execution.failure().stderr(contains("cannot parse owner"));
}
