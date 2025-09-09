// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use assert_cmd::Command;
use std::env::current_dir;

fn sut() -> Command {
    let _ = env_logger::builder().is_test(true).try_init();
    Command::cargo_bin("canopus").expect("Failed to create a command")
}

#[test]
fn validate_own_codeowners_configuration() {
    let current_dir = current_dir().unwrap();
    let project_root = current_dir // tests
        .parent()
        .unwrap() // crates
        .parent()
        .unwrap() // root
        .to_str()
        .unwrap();

    let args = ["validate", "-p", project_root];

    sut().args(args).assert().success();
}
