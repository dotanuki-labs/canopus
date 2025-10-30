// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use assert_cmd::Command;
use predicates::str::contains;
use std::env::current_dir;

fn sut() -> Command {
    let _ = env_logger::builder().is_test(true).try_init();
    assert_cmd::cargo::cargo_bin_cmd!("canopus")
}

fn find_project_root() -> String {
    let current_dir = current_dir().unwrap();
    current_dir // tests
        .parent()
        .unwrap() // crates
        .parent()
        .unwrap() // root
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn self_validate_codeowners_configuration() {
    let project_root = find_project_root();

    let args = ["validate", "-p", project_root.as_str()];

    sut().args(args).assert().success().stdout(contains("No issues found"));
}

#[test]
fn self_repair_codeowners_configuration() {
    let project_root = find_project_root();

    let args = ["repair", "-p", project_root.as_str(), "--remove-lines"];

    sut()
        .args(args)
        .assert()
        .success()
        .stdout(contains("Nothing to repair"));
}
